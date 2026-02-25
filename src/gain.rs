use crate::db;
use rusqlite::Connection;

/// Cost per 1K tokens (GPT-4o input pricing as a universal baseline).
const COST_PER_1K_TOKENS: f64 = 0.005;

pub fn display(conn: &Connection, show_history: bool) {
    let today = db::query_today(conn).unwrap_or(db::TodayStats {
        runs: 0,
        original_tokens: 0,
        cleaned_tokens: 0,
        avg_savings_pct: 0.0,
    });
    let total = db::query_total(conn).unwrap_or(db::TotalStats {
        runs: 0,
        original_tokens: 0,
        cleaned_tokens: 0,
        avg_savings_pct: 0.0,
    });

    let today_saved = today.original_tokens.saturating_sub(today.cleaned_tokens);
    let total_saved = total.original_tokens.saturating_sub(total.cleaned_tokens);
    let today_cost = tokens_to_cost(today_saved);
    let total_cost = tokens_to_cost(total_saved);

    println!("┌───────────────────────────────────────────────────┐");
    println!("│               ITK — Token Savings                  │");
    println!("├────────────────┬──────────────────┬───────────────┤");
    println!("│                │     Today        │   All Time    │");
    println!("├────────────────┼──────────────────┼───────────────┤");
    println!(
        "│ Runs           │ {:>14}   │ {:>11}   │",
        today.runs, total.runs
    );
    println!(
        "│ Tokens in      │ {:>14}   │ {:>11}   │",
        fmt_num(today.original_tokens),
        fmt_num(total.original_tokens)
    );
    println!(
        "│ Tokens out     │ {:>14}   │ {:>11}   │",
        fmt_num(today.cleaned_tokens),
        fmt_num(total.cleaned_tokens)
    );
    println!(
        "│ Tokens saved   │ {:>14}   │ {:>11}   │",
        fmt_num(today_saved),
        fmt_num(total_saved)
    );
    println!(
        "│ Avg savings    │ {:>13.1}%  │ {:>10.1}%  │",
        today.avg_savings_pct, total.avg_savings_pct
    );
    println!(
        "│ Est. cost saved│ {:>14}   │ {:>11}   │",
        format!("${today_cost:.4}"),
        format!("${total_cost:.4}")
    );
    println!("└────────────────┴──────────────────┴───────────────┘");

    if let Ok(by_type) = db::query_by_type(conn) {
        if !by_type.is_empty() {
            println!();
            println!("  By content type (all time):");
            println!("  {:<26} {:>6}  {:>10}", "Type", "Runs", "Avg savings");
            println!("  {}", "─".repeat(46));
            for (ct, runs, avg_pct) in &by_type {
                let display = friendly_type_name(ct);
                println!("  {:<26} {:>6}  {:>9.1}%", display, runs, avg_pct);
            }
        }
    }

    if show_history {
        display_history(conn);
    }
}

fn display_history(conn: &Connection) {
    let records = match db::query_history(conn, 50) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("itk: history query failed: {e}");
            return;
        }
    };

    if records.is_empty() {
        println!("\n  No history yet.");
        return;
    }

    println!();
    println!("  Recent runs (last 50):");
    println!(
        "  {:<20} {:<18} {:>8}  {:>8}  {:>8}",
        "Time", "Type", "In", "Out", "Saved"
    );
    println!("  {}", "─".repeat(68));

    for r in &records {
        let ts_short = r.ts.get(..16).unwrap_or(&r.ts);
        let ct_short = friendly_type_name(&r.content_type);
        println!(
            "  {:<20} {:<18} {:>8}  {:>8}  {:>7.1}%",
            ts_short,
            ct_short,
            fmt_num(r.original_tokens as u64),
            fmt_num(r.cleaned_tokens as u64),
            r.savings_pct
        );
    }
}

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
        .replace(')', "")
        .replace("Unknown", "trace/unknown")
}
