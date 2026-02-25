mod clean;
mod clipboard;
mod db;
mod detect;
mod gain;
mod prompt;

use clap::{Parser, Subcommand};
use std::io::{self, IsTerminal, Read, Write};

#[derive(Parser)]
#[command(
    name = "itk",
    about = "Input Token Killer — compress content before pasting into LLMs",
    long_about = "ITK reads from clipboard (or stdin), cleans and compresses the content,\n\
                  then writes it back to clipboard (or stdout).\n\n\
                  Auto-detects: stack traces, git diffs, logs, JSON, YAML, code.",
    version
)]
struct Cli {
    /// Add a 1-2 line summary header describing the content
    #[arg(short = 's', long = "summary")]
    summary: bool,

    /// Aggressively truncate repeated frames and deep traces
    #[arg(long = "aggressive")]
    aggressive: bool,

    /// Specialised mode for git diff / patch format
    #[arg(long = "diff")]
    diff: bool,

    /// Wrap output in a prompt template: fix | explain | refactor | review | debug
    #[arg(long = "prompt", value_name = "TYPE")]
    prompt_type: Option<String>,

    /// Print token savings inline as a header comment
    #[arg(long = "stats")]
    stats: bool,

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
    },
}

fn main() {
    let cli = Cli::parse();

    // ── Handle subcommands ────────────────────────────────────────────────────
    if let Some(Commands::Gain { history }) = cli.command {
        match db::open() {
            Ok(conn) => gain::display(&conn, history),
            Err(e) => eprintln!("itk: could not open history database: {e}"),
        }
        return;
    }

    // ── Determine input/output mode ───────────────────────────────────────────
    let is_pipe = !io::stdin().is_terminal();

    let input = if is_pipe {
        let mut buf = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buf) {
            eprintln!("itk: failed to read stdin: {e}");
            return;
        }
        buf
    } else {
        match clipboard::read() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("itk: {e}");
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

    // ── Detect → Clean ────────────────────────────────────────────────────────
    let content_type = detect::detect(&input, cli.diff);

    let opts = clean::CleanOptions {
        aggressive: cli.aggressive,
        _diff_mode: cli.diff,
        add_summary: cli.summary,
        prompt_type: cli.prompt_type.as_deref(),
        content_type: content_type.clone(),
    };

    let cleaned = clean::clean(&input, &opts);

    // ── Token accounting ──────────────────────────────────────────────────────
    let original_tokens = estimate_tokens(&input);
    let cleaned_tokens = estimate_tokens(&cleaned);
    let savings_pct = if original_tokens > 0 {
        100u64.saturating_sub(cleaned_tokens * 100 / original_tokens)
    } else {
        0
    };

    // Persist stats (best-effort — never blocks or fails the main path)
    if let Ok(mut conn) = db::open() {
        let _ = db::record_run(
            &mut conn,
            &format!("{content_type:?}"),
            original_tokens,
            cleaned_tokens,
        );
    }

    // ── Build final output ────────────────────────────────────────────────────
    let output = if cli.stats {
        format!(
            "# ITK: {original_tokens} → {cleaned_tokens} tokens (-{savings_pct}%)\n{cleaned}"
        )
    } else {
        cleaned
    };

    // ── Write output ──────────────────────────────────────────────────────────
    if is_pipe {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        if let Err(e) = handle.write_all(output.as_bytes()) {
            eprintln!("itk: stdout write failed: {e}");
        }
        // Ensure output ends with newline for shell friendliness
        if !output.ends_with('\n') {
            let _ = handle.write_all(b"\n");
        }
    } else {
        match clipboard::write(&output) {
            Ok(()) => {
                eprintln!(
                    "itk: clipboard updated  {original_tokens} → {cleaned_tokens} tokens  \
                     (-{savings_pct}%)"
                );
            }
            Err(e) => eprintln!("itk: {e}"),
        }
    }
}

/// Estimate token count: word_count × 1.3, rounded to nearest integer.
/// No LLM or tiktoken needed — accurate enough for savings tracking.
fn estimate_tokens(text: &str) -> u64 {
    let words = text.split_whitespace().count() as f64;
    (words * 1.3).round() as u64
}
