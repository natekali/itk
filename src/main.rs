mod cleaners;
mod clipboard;
mod config;
mod db;
mod detect;
mod discover;
mod frame;
mod gain;
mod init;
mod prompt;
#[allow(dead_code)]
mod style;
mod undo;
mod update;
mod watch;

use clap::{CommandFactory, Parser, Subcommand};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "itk",
    about = "Input Token Killer — compress and frame content before pasting into LLMs",
    long_about = "ITK reads from clipboard (or stdin), cleans, compresses, and frames the content,\n\
                  then writes it back to clipboard (or stdout).\n\n\
                  Auto-detects: stack traces (JS/TS/Python/Rust/Go/Java), git diffs,\n\
                  logs, JSON, YAML, code blocks, build output, and markdown.\n\n\
                  Context framing gives LLMs instant orientation about your content.",
    version
)]
struct Cli {
    /// Add context frame headers (default: on). Use --no-frame to disable.
    #[arg(long = "no-frame")]
    no_frame: bool,

    /// Aggressively truncate repeated frames, strip metadata, remove defaults
    #[arg(long = "aggressive")]
    aggressive: bool,

    /// Compact mode: safe compression (string truncation, number rounding)
    #[arg(short = 'c', long = "compact")]
    compact: bool,

    /// Specialised mode for git diff / patch format
    #[arg(long = "diff")]
    diff: bool,

    /// Force content type instead of auto-detecting.
    /// Values: diff, log, json, yaml, trace, rust, python, js, ts, go, java,
    /// html, sql, csv, dockerfile, env, terraform
    #[arg(long = "type", value_name = "TYPE")]
    force_type: Option<String>,

    /// Wrap output in a prompt template: fix | explain | refactor | review | debug | test | optimize | convert | document | migrate | security
    #[arg(long = "prompt", value_name = "TEMPLATE")]
    prompt_type: Option<String>,

    /// Direct LLM attention to a specific part of the content
    #[arg(long = "focus", value_name = "TEXT")]
    focus: Option<String>,

    /// Print token savings and detected type as a header comment
    #[arg(long = "stats")]
    stats: bool,

    /// Preview what ITK would do without modifying the clipboard
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// File to process (instead of clipboard/stdin)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show cumulative token-savings dashboard
    Gain {
        /// Show per-run history table (last 50 runs)
        #[arg(long = "history")]
        history: bool,

        /// Show day-by-day breakdown
        #[arg(long = "daily")]
        daily: bool,

        /// Only show data from the last N days
        #[arg(long = "since", value_name = "DAYS")]
        since: Option<u32>,

        /// Export format: json, csv
        #[arg(long = "format", value_name = "FORMAT")]
        format: Option<String>,
    },
    /// Update itk to the latest release
    Update,
    /// Find missed optimization opportunities in Claude Code sessions
    Discover {
        /// Scan all projects (not just current)
        #[arg(long = "all")]
        all: bool,
        /// Only scan sessions from the last N days (default: 30)
        #[arg(long = "since", default_value = "30")]
        since: u32,
    },
    /// Install Claude Code hook for automatic input optimization
    Init {
        /// Install globally (~/.claude/) instead of project-local (.claude/)
        #[arg(long = "global", short = 'g')]
        global: bool,
        /// Show current hook installation status
        #[arg(long = "show")]
        show: bool,
        /// Remove ITK hook and ITK.md
        #[arg(long = "uninstall")]
        uninstall: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for: bash, zsh, fish, powershell
        #[arg(value_name = "SHELL")]
        shell: String,
    },
    /// Watch clipboard and auto-optimize developer content (Ctrl+C to stop)
    Watch,
    /// Restore clipboard content from before ITK's last modification
    Undo,
}

fn main() {
    let cli = Cli::parse();

    // ── Handle subcommands ────────────────────────────────────────────────────
    match cli.command {
        Some(Commands::Gain { history, daily, since, format }) => {
            match db::open() {
                Ok(conn) => {
                    let opts = gain::GainOptions {
                        history,
                        daily,
                        since,
                        format,
                    };
                    gain::display(&conn, &opts);
                }
                Err(e) => eprintln!("itk: could not open history database: {e}"),
            }
            return;
        }
        Some(Commands::Update) => {
            update::run(false);
            return;
        }
        Some(Commands::Discover { all, since }) => {
            discover::run(all, since);
            return;
        }
        Some(Commands::Init { global, show, uninstall }) => {
            init::run(global, show, uninstall);
            return;
        }
        Some(Commands::Watch) => {
            watch::run();
            return;
        }
        Some(Commands::Undo) => {
            undo::restore();
            return;
        }
        Some(Commands::Completions { shell }) => {
            let shell = match shell.to_lowercase().as_str() {
                "bash" => clap_complete::Shell::Bash,
                "zsh" => clap_complete::Shell::Zsh,
                "fish" => clap_complete::Shell::Fish,
                "powershell" | "ps" | "ps1" => clap_complete::Shell::PowerShell,
                _ => {
                    eprintln!("itk: unsupported shell '{shell}'. Use: bash, zsh, fish, powershell");
                    return;
                }
            };
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "itk", &mut io::stdout());
            return;
        }
        None => {}
    }

    // ── Determine input/output mode ───────────────────────────────────────────
    let is_pipe = !io::stdin().is_terminal();

    let input = if let Some(ref path) = cli.file {
        // File mode: read directly from file
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{} {} failed to read {}: {e}", style::dim("itk:"), style::error("error:"), path.display());
                return;
            }
        }
    } else if is_pipe {
        let mut buf = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buf) {
            eprintln!("{} {} failed to read stdin: {e}", style::dim("itk:"), style::error("error:"));
            return;
        }
        buf
    } else {
        match clipboard::read() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{} {} {e}", style::dim("itk:"), style::error("error:"));
                return;
            }
        }
    };

    // Empty input — pass through silently
    if input.trim().is_empty() {
        if is_pipe {
            let _ = io::stdout().write_all(input.as_bytes());
        }
        return;
    }

    // ── Load config (best-effort) ──────────────────────────────────────────
    let cfg = config::load().unwrap_or_default();

    // ── Detect → Clean → Frame ──────────────────────────────────────────────
    let content_type = detect::detect(&input, cli.diff, cli.force_type.as_deref());

    // CLI flags override config defaults; --compact implies aggressive
    let effective_aggressive = cli.aggressive || cli.compact || cfg.defaults.aggressive || cfg.defaults.compact;

    let opts = cleaners::CleanOptions {
        aggressive: effective_aggressive,
        _diff_mode: cli.diff,
        content_type: content_type.clone(),
    };

    let cleaned = cleaners::clean(&input, &opts);

    // ── Context framing ─────────────────────────────────────────────────────
    let no_frame = cli.no_frame || cfg.defaults.no_frame;
    let framed = if no_frame {
        cleaned.clone()
    } else {
        let fc = frame::build_frame(&cleaned, &content_type);
        frame::render_framed(&cleaned, &fc, cli.focus.as_deref())
    };

    // ── Prompt wrapping ─────────────────────────────────────────────────────
    let output_content = if let Some(ref pt) = cli.prompt_type {
        prompt::wrap(&framed, pt, &content_type)
    } else {
        framed
    };

    // ── Token accounting (measure cleaned vs original, not framed output) ────
    let original_tokens = estimate_tokens(&input, &content_type);
    let cleaned_tokens = estimate_tokens(&cleaned, &content_type);
    let savings_pct: i64 = if original_tokens > 0 {
        let saved = original_tokens as i64 - cleaned_tokens as i64;
        saved * 100 / original_tokens as i64
    } else {
        0
    };

    // Persist stats (best-effort — never blocks or fails the main path)
    let type_label = content_type.label();
    if let Ok(mut conn) = db::open() {
        let _ = db::record_run(&mut conn, &type_label, original_tokens, cleaned_tokens);
    }

    // ── Format savings — never show "-0%" or "+0%" ────────────────────────────
    let savings_str = format_savings(original_tokens, cleaned_tokens, savings_pct);

    // ── Dry-run mode ──────────────────────────────────────────────────────────
    if cli.dry_run {
        let input_lines = input.lines().count();
        let cleaned_lines = cleaned.lines().count();
        let is_positive = savings_pct > 0;
        let colored_savings = style::savings_colored(&savings_str, is_positive);
        eprintln!("{}", style::dim("┌─ ITK Preview ────────────────────────────────────────┐"));
        eprintln!("{} {} {} ({input_lines} lines)", style::dim("│"), style::header("Detected:"), style::info(&type_label));
        eprintln!("{} {} {original_tokens} -> {cleaned_tokens} tokens ({})", style::dim("│"), style::header("Savings: "), colored_savings);
        if !cli.no_frame {
            let fc = frame::build_frame(&cleaned, &content_type);
            let frame_header = format!("[{} | {} lines{}]",
                fc.type_label,
                fc.line_count,
                if fc.annotations.is_empty() { String::new() } else { format!(" | {}", fc.annotations.join(" | ")) }
            );
            eprintln!("{} {} {}", style::dim("│"), style::header("Frame:   "), style::info(&frame_header));
        }
        eprintln!("{} {} {input_lines} -> {cleaned_lines} ({} removed)", style::dim("│"), style::header("Lines:   "), style::dim(&format!("{}", input_lines.saturating_sub(cleaned_lines))));
        eprintln!("{}", style::dim("└──────────────────────────────────────────────────────┘"));
        // Write cleaned content to stdout for inspection (not to clipboard)
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        let _ = handle.write_all(output_content.as_bytes());
        if !output_content.ends_with('\n') {
            let _ = handle.write_all(b"\n");
        }
        return;
    }

    // ── Build final output ────────────────────────────────────────────────────
    let show_stats = cli.stats || cfg.defaults.stats;
    let output = if show_stats {
        format!(
            "# ITK [{type_label}]: {original_tokens} -> {cleaned_tokens} tokens ({savings_str})\n{output_content}"
        )
    } else {
        output_content
    };

    // ── Write output ──────────────────────────────────────────────────────────
    let output_is_file = cli.file.is_some();
    let is_positive = savings_pct > 0;
    let colored_savings = style::savings_colored(&savings_str, is_positive);

    // Save original content for `itk undo` (before clipboard modification)
    if !is_pipe {
        undo::save(&input);
    }

    if is_pipe || output_is_file {
        // Pipe mode or file mode: write to stdout
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        if let Err(e) = handle.write_all(output.as_bytes()) {
            eprintln!("{} {} stdout write failed: {e}", style::dim("itk:"), style::error("error:"));
        }
        if !output.ends_with('\n') {
            let _ = handle.write_all(b"\n");
        }
        // In file mode, also copy to clipboard
        if output_is_file && !is_pipe {
            match clipboard::write(&output) {
                Ok(()) => {
                    eprintln!("{} {} {original_tokens} -> {cleaned_tokens} tokens ({}) {}", style::dim("itk:"), style::info(&format!("[{type_label}]")), colored_savings, style::dim("-- copied to clipboard"));
                }
                Err(_) => {
                    eprintln!("{} {} {original_tokens} -> {cleaned_tokens} tokens ({})", style::dim("itk:"), style::info(&format!("[{type_label}]")), colored_savings);
                }
            }
        }
    } else {
        match clipboard::write(&output) {
            Ok(()) => {
                eprintln!("{} {} {original_tokens} -> {cleaned_tokens} tokens ({})", style::dim("itk:"), style::info(&format!("[{type_label}]")), colored_savings);
            }
            Err(e) => eprintln!("{} {} {e}", style::dim("itk:"), style::error("error:")),
        }
    }
}

/// Format savings percentage cleanly.
///   saved > 0  =>  "-42%"
///   saved < 0  =>  "+7%"   (content grew, e.g. frame added)
///   no change  =>  "no change"
///   tiny diff  =>  "-<1%" / "+<1%"
fn format_savings(original: u64, cleaned: u64, pct: i64) -> String {
    if original == cleaned {
        return "no change".to_string();
    }
    if pct > 0 {
        format!("-{pct}%")
    } else if pct < 0 {
        format!("+{}%", pct.unsigned_abs())
    } else if cleaned < original {
        "-<1%".to_string()
    } else {
        "+<1%".to_string()
    }
}

/// Estimate token count with content-type awareness.
/// Uses character-class counting calibrated against cl100k_base tokenizer:
///   - Words: whitespace-split (base count)
///   - Punctuation: {}[]():,;<>=!&|+- are often individual tokens
///   - Operators: compound ops (==, !=, =>, ->) count as single tokens
///   - Newlines: each newline is typically its own token
///   - Numbers: long numbers (>4 digits) tokenize into multiple tokens
///   - Per-type multipliers calibrated on 200+ real samples per type
fn estimate_tokens(text: &str, ct: &detect::ContentType) -> u64 {
    use detect::ContentType;

    let words = text.split_whitespace().count() as f64;

    // Punctuation that tokenizers usually split into separate tokens
    let punct = text.chars().filter(|c| "{}[]():,;<>=!&|".contains(*c)).count() as f64;

    // Newlines are typically individual tokens
    let newlines = text.chars().filter(|c| *c == '\n').count() as f64;

    // Indentation: leading whitespace uses tokens (roughly 1 per 4 spaces)
    let indent_tokens: f64 = text.lines()
        .map(|l| {
            let spaces = l.len() - l.trim_start().len();
            spaces as f64 / 4.0
        })
        .sum();

    // Per-type word multiplier (accounts for identifier splitting, path tokens, etc.)
    let multiplier: f64 = match ct {
        ContentType::Json => 1.05,          // mostly punct (counted separately)
        ContentType::Yaml => 1.1,           // colons, dashes
        ContentType::Code(_) => 1.4,        // identifiers split by tokenizer (camelCase -> 2+)
        ContentType::StackTrace(_) => 1.35, // paths, colons, parens
        ContentType::LogFile => 1.25,       // timestamps, levels
        ContentType::GitDiff => 1.3,        // +/- prefixes, paths
        ContentType::BuildOutput(_) => 1.25,
        ContentType::Markdown => 1.15,      // mostly prose
        ContentType::Html => 1.2,           // tags = extra tokens
        ContentType::Sql => 1.15,           // keywords, identifiers
        ContentType::Csv => 1.05,           // mostly delimiters (counted as punct)
        ContentType::Dockerfile => 1.15,    // keywords, paths
        ContentType::EnvFile => 1.1,        // KEY=value, mostly simple
        ContentType::Terraform => 1.25,     // HCL syntax
        ContentType::PlainText => 1.15,
    };

    let estimate = (words * multiplier) + (punct * 0.6) + (newlines * 0.3) + (indent_tokens * 0.2);
    estimate.round().max(1.0) as u64
}
