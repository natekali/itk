use crate::detect::{self, ContentType};
use crate::style;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Run the discover command: scan Claude Code transcripts for missed savings.
pub fn run(all_projects: bool, since_days: u32) {
    let projects_dir = resolve_projects_dir();
    if !projects_dir.exists() {
        eprintln!("{} {} no Claude Code sessions found at {}", style::dim("itk:"), style::error("error:"), projects_dir.display());
        eprintln!("  (expected: ~/.claude/projects/)");
        return;
    }

    let cutoff = chrono_cutoff(since_days);

    // Collect all JSONL files
    let jsonl_files = if all_projects {
        find_all_jsonl(&projects_dir, &cutoff)
    } else {
        // Current project only
        let cwd = std::env::current_dir().unwrap_or_default();
        let project_slug = cwd_to_slug(&cwd);
        let project_dir = projects_dir.join(&project_slug);
        if !project_dir.exists() {
            eprintln!("{} no Claude Code sessions found for current project", style::dim("itk:"));
            eprintln!("  (looked in: {})", style::dim(&project_dir.display().to_string()));
            eprintln!("  Tip: use {} to scan all projects", style::info("--all"));
            return;
        }
        find_all_jsonl(&project_dir, &cutoff)
    };

    if jsonl_files.is_empty() {
        eprintln!("{} no sessions found in the last {since_days} days", style::dim("itk:"));
        return;
    }

    // Scan and classify
    let mut stats = DiscoverStats::default();
    let mut sessions_scanned = 0u32;

    for path in &jsonl_files {
        sessions_scanned += 1;
        scan_jsonl(path, &mut stats);
    }

    // Display results
    display_results(&stats, sessions_scanned, since_days);
}

#[derive(Default)]
struct DiscoverStats {
    by_type: BTreeMap<String, TypeStats>,
    total_messages: u32,
    optimizable_messages: u32,
}

#[derive(Default)]
struct TypeStats {
    count: u32,
    est_tokens: u64,
    est_savings: u64,
}

fn scan_jsonl(path: &PathBuf, stats: &mut DiscoverStats) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        // Quick filter: only process user messages
        if !line.contains("\"type\":\"user\"") {
            continue;
        }

        let text = extract_user_text(line);
        if text.is_empty() || text.len() < 300 {
            continue;
        }

        stats.total_messages += 1;

        // Detect content type
        let ct = detect::detect(&text, false, None);

        // Skip plain text — nothing to optimize
        if ct == ContentType::PlainText {
            continue;
        }

        stats.optimizable_messages += 1;

        let label = ct.label();
        let est_tokens = estimate_tokens_simple(&text);
        let est_savings = estimate_savings(&ct, est_tokens);

        let entry = stats.by_type.entry(label).or_default();
        entry.count += 1;
        entry.est_tokens += est_tokens;
        entry.est_savings += est_savings;
    }
}

/// Extract user message text from a JSONL line.
fn extract_user_text(line: &str) -> String {
    // Parse minimal JSON to extract content
    // The format is: {"type":"user","message":{"content":"text" or [{"type":"text","text":"..."}]}}
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(val) => {
            let content = &val["message"]["content"];
            match content {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(arr) => {
                    // Concatenate all text parts
                    arr.iter()
                        .filter_map(|item| {
                            if item["type"] == "text" {
                                item["text"].as_str().map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                _ => String::new(),
            }
        }
        Err(_) => String::new(),
    }
}

/// Simple token estimation (words * 1.3)
fn estimate_tokens_simple(text: &str) -> u64 {
    let words = text.split_whitespace().count() as f64;
    (words * 1.3).round() as u64
}

/// Estimate potential savings based on content type.
fn estimate_savings(ct: &ContentType, tokens: u64) -> u64 {
    let pct = match ct {
        ContentType::StackTrace(_) => 0.75,   // 75% savings typical
        ContentType::LogFile => 0.80,          // 80% via dedup
        ContentType::GitDiff => 0.55,          // 55% context trimming
        ContentType::Json => 0.37,             // 37% pruning/dedup
        ContentType::Yaml => 0.25,             // 25% comment/default removal
        ContentType::Code(_) => 0.15,          // 15% doc/import collapse
        ContentType::BuildOutput(_) => 0.80,   // 80% noise removal
        ContentType::Markdown => 0.27,         // 27% section/badge removal
        ContentType::Html => 0.60,             // 60% tag stripping
        ContentType::Sql => 0.15,              // 15% comment/whitespace
        ContentType::Csv => 0.70,              // 70% row truncation
        ContentType::Dockerfile => 0.20,       // 20% comment removal
        ContentType::EnvFile => 0.10,          // 10% masking (main value is security)
        ContentType::Terraform => 0.20,        // 20% comment/default removal
        ContentType::PlainText => 0.0,
    };
    (tokens as f64 * pct).round() as u64
}

fn display_results(stats: &DiscoverStats, sessions: u32, since_days: u32) {
    eprintln!();
    eprintln!("{}", style::header("ITK Discover -- Missed Savings"));
    eprintln!("{}", style::dim("===================================================="));
    eprintln!("Scanned: {} sessions (last {} days), {} user messages with pasted content",
        sessions, since_days, stats.total_messages);
    eprintln!();

    if stats.optimizable_messages == 0 {
        eprintln!("No optimizable content found. Either:");
        eprintln!("  - Your messages were too short to optimize (<300 chars)");
        eprintln!("  - Content was plain text (no structured data detected)");
        eprintln!();
        return;
    }

    eprintln!("{}", style::header("CONTENT YOU COULD HAVE OPTIMIZED"));
    eprintln!("{}", style::dim("----------------------------------------------------"));
    eprintln!("{} {:>6} {:>12} {:>16}",
        style::info(&format!("{:<24}", "Content Type")),
        style::info("Count"),
        style::info("Est. Tokens"),
        style::info("Potential Savings"));
    eprintln!("{}", style::dim("----------------------------------------------------"));

    let mut total_tokens = 0u64;
    let mut total_savings = 0u64;

    for (label, ts) in &stats.by_type {
        total_tokens += ts.est_tokens;
        total_savings += ts.est_savings;

        let pct = if ts.est_tokens > 0 {
            (ts.est_savings as f64 / ts.est_tokens as f64 * 100.0).round() as u64
        } else {
            0
        };

        let savings_str = format!("~{}K (-{}%)", format_k(ts.est_savings), pct);
        eprintln!("{} {:>6} {:>10}K {:>16}",
            style::info(&format!("{:<24}", label)),
            ts.count,
            format_k(ts.est_tokens),
            style::savings_colored(&savings_str, true)
        );
    }

    eprintln!("{}", style::dim("----------------------------------------------------"));

    let total_pct = if total_tokens > 0 {
        (total_savings as f64 / total_tokens as f64 * 100.0).round() as u64
    } else {
        0
    };

    let total_str = format!("~{}K tokens saveable (-{}%)", format_k(total_savings), total_pct);
    eprintln!("{} {} messages -> {}",
        style::header("Total:"),
        stats.optimizable_messages,
        style::savings_colored(&total_str, true)
    );
    eprintln!();

    // Actionable tip
    eprintln!("Run {} to optimize automatically.", style::success("itk init --global"));
    eprintln!();
}

fn format_k(tokens: u64) -> String {
    if tokens >= 1000 {
        format!("{:.1}", tokens as f64 / 1000.0)
    } else {
        format!("~{}", tokens)
    }
}

// ── Path resolution ─────────────────────────────────────────────────────────

fn resolve_projects_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".claude").join("projects")
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile).join(".claude").join("projects")
    } else {
        PathBuf::from(".claude").join("projects")
    }
}

fn cwd_to_slug(cwd: &std::path::Path) -> String {
    // Claude Code uses directory path with separators replaced by --
    let path_str = cwd.display().to_string();
    // Replace path separators and colons
    path_str
        .replace('\\', "-")
        .replace('/', "-")
        .replace(':', "")
}

fn find_all_jsonl(dir: &PathBuf, cutoff: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    scan_dir_for_jsonl(dir, &mut files, cutoff, 3);
    files
}

fn scan_dir_for_jsonl(dir: &PathBuf, files: &mut Vec<PathBuf>, _cutoff: &str, depth: u32) {
    if depth == 0 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir_for_jsonl(&path, files, _cutoff, depth - 1);
        } else if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            // Check modification time against cutoff
            if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let age = std::time::SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default();
                    // Parse cutoff days from the string (we pass days as string)
                    let cutoff_secs: u64 = _cutoff.parse::<u64>().unwrap_or(30) * 86400;
                    if age.as_secs() <= cutoff_secs {
                        files.push(path);
                    }
                }
            }
        }
    }
}

fn chrono_cutoff(days: u32) -> String {
    // Return days as string — used by scan_dir_for_jsonl for age comparison
    days.to_string()
}
