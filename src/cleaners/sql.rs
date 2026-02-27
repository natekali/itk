use regex::Regex;

/// Clean SQL: normalize whitespace, uppercase keywords, remove comments.
pub fn clean_sql(s: &str, aggressive: bool) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0usize;

    for line in s.lines() {
        let trimmed = line.trim();

        // Skip blank lines (collapse)
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // Skip SQL comments
        if trimmed.starts_with("--") {
            if !aggressive {
                out.push(trimmed.to_string());
            }
            continue;
        }

        // Skip block comments (/* ... */) on single line
        if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            continue;
        }

        // Uppercase SQL keywords for readability
        let line = uppercase_sql_keywords(trimmed);
        out.push(line);
    }

    // In aggressive mode, also collapse multi-line INSERT VALUES into summary
    let result = out.join("\n");
    if aggressive {
        collapse_insert_values(&result)
    } else {
        result
    }
}

fn uppercase_sql_keywords(line: &str) -> String {
    let keywords = [
        "select", "from", "where", "join", "left join", "right join",
        "inner join", "outer join", "cross join", "on", "and", "or",
        "not", "in", "between", "like", "order by", "group by",
        "having", "limit", "offset", "insert into", "values",
        "update", "set", "delete from", "create table", "alter table",
        "drop table", "create index", "drop index", "as", "distinct",
        "union", "union all", "except", "intersect", "exists",
        "case", "when", "then", "else", "end", "begin", "commit",
        "rollback", "with", "returning", "into", "is null", "is not null",
        "asc", "desc", "cascade", "primary key", "foreign key",
        "references", "default", "not null", "unique", "check",
        "constraint", "index", "if exists", "if not exists",
    ];

    // Simple word-boundary keyword replacement
    // Only replace if the word appears at a word boundary (not inside identifiers)
    let mut result = line.to_string();
    for kw in &keywords {
        let re = keyword_regex(kw);
        result = re.replace_all(&result, kw.to_uppercase().as_str()).into_owned();
    }
    result
}

fn keyword_regex(keyword: &str) -> Regex {
    // Build a case-insensitive word-boundary regex
    let escaped = regex::escape(keyword);
    Regex::new(&format!(r"(?i)\b{}\b", escaped)).unwrap()
}

/// Collapse multi-row INSERT VALUES into a summary
fn collapse_insert_values(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut in_values = false;
    let mut values_count = 0usize;

    while i < lines.len() {
        let upper = lines[i].trim().to_uppercase();

        if upper.contains("VALUES") {
            in_values = true;
            values_count = 0;
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }

        if in_values {
            let trimmed = lines[i].trim();
            // A values row typically starts with ( and ends with ), or ),
            if trimmed.starts_with('(') {
                values_count += 1;
                if values_count <= 3 {
                    out.push(lines[i].to_string());
                } else if values_count == 4 {
                    out.push(format!("  -- ... [VALUES rows truncated by itk]"));
                }
                // Check if this is the last values row (ends with ; or no trailing comma)
                if trimmed.ends_with(';') || (!trimmed.ends_with(',') && !trimmed.ends_with("),(")) {
                    if values_count > 3 {
                        out.push(format!("  -- ({} total rows)", values_count));
                    }
                    in_values = false;
                }
            } else {
                in_values = false;
                if values_count > 3 {
                    out.push(format!("  -- ({} total rows)", values_count));
                }
                out.push(lines[i].to_string());
            }
        } else {
            out.push(lines[i].to_string());
        }
        i += 1;
    }

    if in_values && values_count > 3 {
        out.push(format!("  -- ({} total rows)", values_count));
    }

    out.join("\n")
}
