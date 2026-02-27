use crate::db;
use crate::style;
use rusqlite::Connection;

/// Cost per 1K tokens (GPT-4o input pricing as a universal baseline).
const COST_PER_1K_TOKENS: f64 = 0.005;

pub struct GainOptions {
    pub history: bool,
    pub daily: bool,
    pub since: Option<u32>,
    pub format: Option<String>,
}

pub fn display(conn: &Connection, opts: &GainOptions) {
    // ── Export mode: JSON or CSV ──────────────────────────────────────────────
    if let Some(ref fmt) = opts.format {
        match fmt.to_lowercase().as_str() {
            "json" => export_json(conn, opts.since),
            "csv" => export_csv(conn, opts.since),
            _ => eprintln!("{} unsupported format '{}'. Use: json, csv", style::dim("itk:"), fmt),
        }
        return;
    }

    // ── Daily breakdown mode ─────────────────────────────────────────────────
    if opts.daily {
        display_daily(conn, opts.since.unwrap_or(30));
        return;
    }

    // ── Standard dashboard ───────────────────────────────────────────────────
    let today = db::query_today(conn).unwrap_or(db::TodayStats {
        runs: 0,
        original_tokens: 0,
        cleaned_tokens: 0,
        avg_savings_pct: 0.0,
    });

    // If --since is set, show "Period" instead of "All Time"
    let (period_label, period_stats) = if let Some(days) = opts.since {
        let stats = db::query_range(conn, days).unwrap_or(db::TotalStats {
            runs: 0,
            original_tokens: 0,
            cleaned_tokens: 0,
            avg_savings_pct: 0.0,
        });
        (format!("Last {days}d"), stats)
    } else {
        let stats = db::query_total(conn).unwrap_or(db::TotalStats {
            runs: 0,
            original_tokens: 0,
            cleaned_tokens: 0,
            avg_savings_pct: 0.0,
        });
        ("All Time".to_string(), stats)
    };

    let today_saved = today.original_tokens.saturating_sub(today.cleaned_tokens);
    let period_saved = period_stats.original_tokens.saturating_sub(period_stats.cleaned_tokens);
    let today_cost = tokens_to_cost(today_saved);
    let period_cost = tokens_to_cost(period_saved);

    let d = |s: &str| style::out_dim(s);
    let h = |s: &str| style::out_header(s);

    println!("{}", d("+-------------------------------------------------+"));
    println!("{} {} {}", d("|"), h("              ITK -- Token Savings              "), d("|"));
    println!("{}", d("+----------------+------------------+--------------+"));
    println!(
        "{} {:14} {} {:>16} {} {:>12} {}",
        d("|"), "", d("|"), style::out_label("Today"), d("|"), style::out_label(&period_label), d("|")
    );
    println!("{}", d("+----------------+------------------+--------------+"));
    println!(
        "{} {} {} {:>16} {} {:>12} {}",
        d("|"), style::out_label("Runs          "), d("|"), today.runs, d("|"), period_stats.runs, d("|")
    );
    println!(
        "{} {} {} {:>16} {} {:>12} {}",
        d("|"), style::out_label("Tokens in     "), d("|"), fmt_num(today.original_tokens), d("|"), fmt_num(period_stats.original_tokens), d("|")
    );
    println!(
        "{} {} {} {:>16} {} {:>12} {}",
        d("|"), style::out_label("Tokens out    "), d("|"), fmt_num(today.cleaned_tokens), d("|"), fmt_num(period_stats.cleaned_tokens), d("|")
    );
    println!(
        "{} {} {} {:>16} {} {:>12} {}",
        d("|"), style::out_label("Tokens saved  "), d("|"),
        style::out_savings(&fmt_num(today_saved), today_saved > 0), d("|"),
        style::out_savings(&fmt_num(period_saved), period_saved > 0), d("|")
    );
    println!(
        "{} {} {} {:>15.1}% {} {:>11.1}% {}",
        d("|"), style::out_label("Avg savings   "), d("|"),
        today.avg_savings_pct, d("|"),
        period_stats.avg_savings_pct, d("|")
    );
    println!(
        "{} {} {} {:>16} {} {:>12} {}",
        d("|"), style::out_label("Est. cost saved"), d("|"),
        style::out_savings(&format!("${today_cost:.4}"), today_cost > 0.0), d("|"),
        style::out_savings(&format!("${period_cost:.4}"), period_cost > 0.0), d("|")
    );
    println!("{}", d("+----------------+------------------+--------------+"));

    if let Ok(by_type) = db::query_by_type(conn) {
        if !by_type.is_empty() {
            println!();
            println!("  {}:", style::out_header("By content type (all time)"));
            println!("  {} {:>6}  {:>10}",
                style::out_label(&format!("{:<26}", "Type")),
                style::out_label("Runs"),
                style::out_label("Avg savings")
            );
            println!("  {}", style::out_dim(&"-".repeat(46)));
            for (ct, runs, avg_pct) in &by_type {
                let display = friendly_type_name(ct);
                let pct_str = format!("{avg_pct:.1}%");
                let colored_pct = style::out_savings(&pct_str, *avg_pct > 0.0);
                println!("  {} {:>6}  {:>10}",
                    style::out_label(&format!("{:<26}", display)),
                    runs,
                    colored_pct
                );
            }
        }
    }

    if opts.history {
        display_history(conn, opts.since);
    }
}

fn display_daily(conn: &Connection, since_days: u32) {
    let days = match db::query_daily(conn, since_days) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{} daily query failed: {e}", style::dim("itk:"));
            return;
        }
    };

    if days.is_empty() {
        println!("  No runs in the last {} days.", since_days);
        return;
    }

    println!("{}", style::out_header(&format!("ITK -- Daily Breakdown (last {} days)", since_days)));
    println!("{}", style::out_dim(&"=".repeat(70)));
    println!(
        "  {} {:>6}  {:>10}  {:>10}  {:>10}  {:>8}",
        style::out_label(&format!("{:<12}", "Date")),
        style::out_label("Runs"),
        style::out_label("Tokens in"),
        style::out_label("Tokens out"),
        style::out_label("Saved"),
        style::out_label("Avg %")
    );
    println!("  {}", style::out_dim(&"-".repeat(62)));

    let mut total_runs: u64 = 0;
    let mut total_in: u64 = 0;
    let mut total_out: u64 = 0;

    for day in &days {
        let saved = day.original_tokens.saturating_sub(day.cleaned_tokens);
        let pct_str = format!("{:.1}%", day.avg_savings_pct);
        let colored_pct = style::out_savings(&pct_str, day.avg_savings_pct > 0.0);
        println!(
            "  {:<12} {:>6}  {:>10}  {:>10}  {:>10}  {:>8}",
            day.date,
            day.runs,
            fmt_num(day.original_tokens),
            fmt_num(day.cleaned_tokens),
            style::out_savings(&fmt_num(saved), saved > 0),
            colored_pct
        );
        total_runs += day.runs;
        total_in += day.original_tokens;
        total_out += day.cleaned_tokens;
    }

    let total_saved = total_in.saturating_sub(total_out);
    let total_avg = if total_in > 0 {
        (total_saved as f64 / total_in as f64) * 100.0
    } else {
        0.0
    };

    println!("  {}", style::out_dim(&"-".repeat(62)));
    let pct_str = format!("{:.1}%", total_avg);
    println!(
        "  {} {:>6}  {:>10}  {:>10}  {:>10}  {:>8}",
        style::out_header(&format!("{:<12}", "TOTAL")),
        total_runs,
        fmt_num(total_in),
        fmt_num(total_out),
        style::out_savings(&fmt_num(total_saved), total_saved > 0),
        style::out_savings(&pct_str, total_avg > 0.0)
    );
}

fn display_history(conn: &Connection, since: Option<u32>) {
    let records = if let Some(days) = since {
        db::query_history_since(conn, days, 50)
    } else {
        db::query_history(conn, 50)
    };

    let records = match records {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} history query failed: {e}", style::dim("itk:"));
            return;
        }
    };

    if records.is_empty() {
        println!("\n  No history yet.");
        return;
    }

    println!();
    println!("  {}:", style::out_header("Recent runs (last 50)"));
    println!(
        "  {} {} {:>8}  {:>8}  {:>8}",
        style::out_label(&format!("{:<20}", "Time")),
        style::out_label(&format!("{:<18}", "Type")),
        style::out_label("In"),
        style::out_label("Out"),
        style::out_label("Saved")
    );
    println!("  {}", style::out_dim(&"-".repeat(68)));

    for r in &records {
        let ts_short = r.ts.get(..16).unwrap_or(&r.ts);
        let ct_short = friendly_type_name(&r.content_type);
        let pct_str = format!("{:.1}%", r.savings_pct);
        let colored_pct = style::out_savings(&pct_str, r.savings_pct > 0.0);
        println!(
            "  {} {} {:>8}  {:>8}  {:>8}",
            style::out_dim(&format!("{:<20}", ts_short)),
            style::out_label(&format!("{:<18}", ct_short)),
            fmt_num(r.original_tokens as u64),
            fmt_num(r.cleaned_tokens as u64),
            colored_pct
        );
    }
}

// ── Export formats ────────────────────────────────────────────────────────────

fn export_json(conn: &Connection, since: Option<u32>) {
    let records = if let Some(days) = since {
        db::query_all_since(conn, days)
    } else {
        db::query_all_since(conn, 36500) // ~100 years = all time
    };

    let records = match records {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} export query failed: {e}", style::dim("itk:"));
            return;
        }
    };

    println!("[");
    for (i, r) in records.iter().enumerate() {
        let comma = if i + 1 < records.len() { "," } else { "" };
        let ct = r.content_type.replace('\"', "\\\"");
        let ts = r.ts.replace('\"', "\\\"");
        println!(
            "  {{\"ts\":\"{}\",\"content_type\":\"{}\",\"original_tokens\":{},\"cleaned_tokens\":{},\"savings_pct\":{:.1}}}{}",
            ts, ct, r.original_tokens, r.cleaned_tokens, r.savings_pct, comma
        );
    }
    println!("]");
}

fn export_csv(conn: &Connection, since: Option<u32>) {
    let records = if let Some(days) = since {
        db::query_all_since(conn, days)
    } else {
        db::query_all_since(conn, 36500)
    };

    let records = match records {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} export query failed: {e}", style::dim("itk:"));
            return;
        }
    };

    println!("ts,content_type,original_tokens,cleaned_tokens,savings_pct");
    for r in &records {
        let ct = if r.content_type.contains(',') {
            format!("\"{}\"", r.content_type)
        } else {
            r.content_type.clone()
        };
        println!(
            "{},{},{},{},{:.1}",
            r.ts, ct, r.original_tokens, r.cleaned_tokens, r.savings_pct
        );
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn tokens_to_cost(tokens: u64) -> f64 {
    tokens as f64 / 1000.0 * COST_PER_1K_TOKENS
}

fn fmt_num(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn friendly_type_name(ct: &str) -> String {
    ct.replace("StackTrace(", "trace/")
        .replace("Code(\"", "code/")
        .replace("BuildOutput(\"", "build/")
        .replace("LogFile", "log")
        .replace("GitDiff", "git-diff")
        .replace("PlainText", "text")
        .replace("Markdown", "markdown")
        .replace("Html", "html")
        .replace("Sql", "sql")
        .replace("Csv", "csv")
        .replace("Dockerfile", "dockerfile")
        .replace("EnvFile", "env")
        .replace("Terraform", "terraform")
        .replace("Json", "json")
        .replace("Yaml", "yaml")
        .replace("Unknown", "unknown")
        .replace("\")", "")
        .replace(')', "")
        .to_lowercase()
}
