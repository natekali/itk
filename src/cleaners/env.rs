use regex::Regex;
use std::sync::OnceLock;

/// Clean .env files: MASK SECRET VALUES to prevent accidental leaking into LLM context.
pub fn clean_env(s: &str, aggressive: bool) -> String {
    let mut out = Vec::new();
    let mut blank_run = 0usize;

    for line in s.lines() {
        let trimmed = line.trim();

        // Blank line handling
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // Always strip comments — they waste tokens for LLMs
        if trimmed.starts_with('#') {
            continue;
        }

        // Parse KEY=value
        if let Some((key, value)) = parse_env_line(trimmed) {
            if is_secret_key(&key) {
                // Always mask secret values
                out.push(format!("{key}=***"));
            } else if aggressive && value.len() > 100 {
                // Truncate very long values in aggressive mode
                out.push(format!("{key}={}...[{} chars]", &value[..50], value.len()));
            } else {
                out.push(format!("{key}={value}"));
            }
        } else {
            // Not a KEY=value line — pass through
            out.push(trimmed.to_string());
        }
    }

    out.join("\n")
}

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let re = re_env_kv();
    re.captures(line).map(|caps| {
        let key = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let value = caps.get(2).map_or("", |m| m.as_str()).to_string();
        // Strip surrounding quotes from value
        let value = value.trim_matches('"').trim_matches('\'').to_string();
        (key, value)
    })
}

fn re_env_kv() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Matches: KEY=value, export KEY=value, KEY="value", KEY='value'
        Regex::new(r#"^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$"#).unwrap()
    })
}

/// Detect if a key name looks like it contains a secret.
fn is_secret_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    let secret_patterns = [
        "SECRET", "KEY", "TOKEN", "PASSWORD", "PASSWD", "PWD",
        "API_KEY", "APIKEY", "AUTH", "CREDENTIAL", "PRIVATE",
        "ACCESS_KEY", "SIGNING", "ENCRYPTION", "CERT",
        "DSN", "DATABASE_URL", "DB_PASS", "DB_PASSWORD",
        "STRIPE", "SENDGRID", "TWILIO", "AWS_SECRET",
        "GITHUB_TOKEN", "NPM_TOKEN", "FIREBASE",
        "JWT", "SESSION_SECRET", "COOKIE_SECRET",
        "SMTP_PASS", "MAIL_PASSWORD",
    ];
    secret_patterns.iter().any(|p| upper.contains(p))
}
