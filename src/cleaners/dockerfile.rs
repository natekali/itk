/// Clean Dockerfile: strip comments, collapse multi-line RUN, detect stages.
pub fn clean_dockerfile(s: &str, aggressive: bool) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0usize;
    let mut in_multiline_run = false;
    let mut run_commands: Vec<String> = Vec::new();

    for line in s.lines() {
        let trimmed = line.trim();

        // Handle multi-line RUN continuation
        if in_multiline_run {
            let cmd = trimmed.trim_end_matches('\\').trim();
            if !cmd.is_empty() && !cmd.starts_with('#') {
                run_commands.push(cmd.to_string());
            }
            // Check if continuation ends (no trailing \)
            if !trimmed.ends_with('\\') {
                // Flush the RUN block
                if aggressive && run_commands.len() > 5 {
                    out.push(format!("RUN {} && \\", run_commands[0]));
                    out.push(format!("    {} && \\", run_commands[1]));
                    out.push(format!("    # ... [{} more commands] && \\", run_commands.len() - 3));
                    out.push(format!("    {}", run_commands.last().unwrap()));
                } else {
                    // Emit all commands in a clean multi-line format
                    for (i, cmd) in run_commands.iter().enumerate() {
                        if i == 0 {
                            if run_commands.len() > 1 {
                                out.push(format!("RUN {cmd} && \\"));
                            } else {
                                out.push(format!("RUN {cmd}"));
                            }
                        } else if i < run_commands.len() - 1 {
                            out.push(format!("    {cmd} && \\"));
                        } else {
                            out.push(format!("    {cmd}"));
                        }
                    }
                }
                run_commands.clear();
                in_multiline_run = false;
            }
            continue;
        }

        // Skip blank lines (collapse)
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // Always strip comments — they waste tokens for LLM context
        if trimmed.starts_with('#') {
            continue;
        }

        // Detect multi-line RUN
        if trimmed.starts_with("RUN ") && trimmed.ends_with('\\') {
            in_multiline_run = true;
            let cmd = trimmed.strip_prefix("RUN ").unwrap().trim_end_matches('\\').trim();
            // Split on && to get individual commands
            for part in cmd.split("&&") {
                let part = part.trim();
                if !part.is_empty() {
                    run_commands.push(part.to_string());
                }
            }
            continue;
        }

        // Single-line RUN: also split on && in aggressive mode
        if trimmed.starts_with("RUN ") && aggressive {
            let cmd = trimmed.strip_prefix("RUN ").unwrap().trim();
            let parts: Vec<&str> = cmd.split("&&").map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
            if parts.len() > 5 {
                out.push(format!("RUN {} && \\", parts[0]));
                out.push(format!("    {} && \\", parts[1]));
                out.push(format!("    # ... [{} more commands] && \\", parts.len() - 3));
                out.push(format!("    {}", parts.last().unwrap()));
            } else {
                out.push(trimmed.to_string());
            }
            continue;
        }

        out.push(trimmed.to_string());
    }

    // Flush any remaining multi-line RUN
    if !run_commands.is_empty() {
        for (i, cmd) in run_commands.iter().enumerate() {
            if i == 0 {
                out.push(format!("RUN {cmd}"));
            } else {
                out.push(format!("    {cmd}"));
            }
        }
    }

    out.join("\n")
}
