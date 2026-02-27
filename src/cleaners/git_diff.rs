use regex::Regex;
use std::sync::OnceLock;

fn re_diff_hunk() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^@@ .+ @@").unwrap())
}

fn re_diff_file_header() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?:diff --git|--- |\+\+\+ |index |Binary files)").unwrap())
}

pub fn clean_git_diff(s: &str, aggressive: bool) -> String {
    let keep: usize = if aggressive { 1 } else { 2 };
    let mut out: Vec<String> = Vec::new();
    let mut in_hunk = false;
    let mut ctx_window: Vec<String> = Vec::new();
    let mut last_was_change = false;

    // Minimum lines to omit before inserting a summary line (must save at least 2 lines net)
    let min_omit = 3;

    let flush_pre_context = |out: &mut Vec<String>, ctx: &mut Vec<String>, keep: usize| {
        let omittable = ctx.len().saturating_sub(keep);
        if omittable >= min_omit {
            out.push(format!(" ... [{} context lines omitted]", omittable));
            let start = ctx.len() - keep;
            for l in ctx.drain(start..) {
                out.push(l);
            }
        } else {
            out.append(ctx);
        }
        ctx.clear();
    };

    let flush_post_context = |out: &mut Vec<String>, ctx: &mut Vec<String>, keep: usize| {
        let omittable = ctx.len().saturating_sub(keep);
        if omittable >= min_omit {
            for l in ctx.drain(..keep) {
                out.push(l);
            }
            out.push(format!(" ... [{} context lines omitted]", ctx.len()));
        } else {
            out.append(ctx);
        }
        ctx.clear();
    };

    for line in s.lines() {
        if re_diff_file_header().is_match(line) {
            if last_was_change {
                flush_post_context(&mut out, &mut ctx_window, keep);
            } else {
                ctx_window.clear();
            }
            out.push(line.to_string());
            in_hunk = false;
            last_was_change = false;
            continue;
        }

        if re_diff_hunk().is_match(line) {
            if last_was_change {
                flush_post_context(&mut out, &mut ctx_window, keep);
            } else {
                ctx_window.clear();
            }
            out.push(line.to_string());
            in_hunk = true;
            last_was_change = false;
            continue;
        }

        if !in_hunk {
            out.push(line.to_string());
            continue;
        }

        match line.chars().next() {
            Some('+') | Some('-') => {
                flush_pre_context(&mut out, &mut ctx_window, keep);
                out.push(line.to_string());
                last_was_change = true;
            }
            Some(' ') => {
                ctx_window.push(line.to_string());
            }
            _ => {
                if last_was_change {
                    flush_post_context(&mut out, &mut ctx_window, keep);
                } else {
                    ctx_window.clear();
                }
                out.push(line.to_string());
                last_was_change = false;
            }
        }
    }

    if last_was_change {
        flush_post_context(&mut out, &mut ctx_window, keep);
    } else {
        ctx_window.clear();
    }

    out.join("\n")
}
