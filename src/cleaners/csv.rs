/// Clean CSV: show header + first N rows + summary.
pub fn clean_csv(s: &str, aggressive: bool) -> String {
    let lines: Vec<&str> = s.lines().collect();

    if lines.is_empty() {
        return s.to_string();
    }

    let max_rows = if aggressive { 3 } else { 5 };
    let total_rows = lines.len().saturating_sub(1); // minus header

    // Header is always shown
    let mut out = Vec::new();
    out.push(lines[0].to_string());

    // Show first N data rows
    let show_count = total_rows.min(max_rows);
    for line in lines.iter().skip(1).take(show_count) {
        out.push(line.to_string());
    }

    // If there are more rows, add a summary
    let remaining = total_rows.saturating_sub(show_count);
    if remaining > 0 {
        out.push(format!("... [{remaining} more rows]"));
    }

    // Detect column count from header (used by frame annotations)
    let _col_count = lines[0].split(',').count();

    // In aggressive mode, also truncate long cell values
    if aggressive {
        out = out.iter().map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            let truncated: Vec<String> = fields.iter().map(|f| {
                let trimmed = f.trim();
                if trimmed.len() > 50 {
                    format!("{}...", &trimmed[..47])
                } else {
                    trimmed.to_string()
                }
            }).collect();
            truncated.join(",")
        }).collect();
    }

    out.join("\n")
}
