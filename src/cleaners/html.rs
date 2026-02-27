use regex::Regex;
use std::sync::OnceLock;

/// Clean HTML/XML content: strip tags, extract text, remove style/script blocks.
pub fn clean_html(s: &str, aggressive: bool) -> String {
    // Pass 1: remove <script> and <style> blocks entirely
    let s = re_script_block().replace_all(s, "").into_owned();
    let s = re_style_block().replace_all(&s, "").into_owned();

    // Pass 2: remove HTML comments
    let s = re_html_comment().replace_all(&s, "").into_owned();

    // Pass 3: extract text content from tags
    let mut out = Vec::new();
    let mut blank_run = 0usize;

    for line in s.lines() {
        let trimmed = line.trim();

        // Skip empty lines (collapse runs)
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank_run = 0;

        // In aggressive mode, strip ALL tags and extract pure text
        if aggressive {
            let text = re_html_tag().replace_all(trimmed, "").trim().to_string();
            if !text.is_empty() {
                // Decode common HTML entities
                let text = decode_entities(&text);
                out.push(text);
            }
        } else {
            // Non-aggressive: simplify but keep structure
            // Remove inline style/class attributes
            let cleaned = re_html_attrs().replace_all(trimmed, "").to_string();
            let cleaned = cleaned.trim();
            if !cleaned.is_empty() {
                out.push(cleaned.to_string());
            }
        }
    }

    // Collapse excessive blank lines
    super::collapse_blank_lines(&out.join("\n"), 1)
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn re_script_block() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?si)<script[^>]*>.*?</script>").unwrap())
}

fn re_style_block() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?si)<style[^>]*>.*?</style>").unwrap())
}

fn re_html_comment() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").unwrap())
}

fn re_html_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<[^>]+>").unwrap())
}

fn re_html_attrs() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Remove class="...", style="...", id="..." attributes
    R.get_or_init(|| {
        Regex::new(r#"\s+(?:class|style|id|data-[\w-]+|aria-[\w-]+|role)\s*=\s*"[^"]*""#).unwrap()
    })
}
