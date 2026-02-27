use regex::Regex;
use std::sync::OnceLock;

/// Clean HTML/XML content: strip tags, extract text, remove style/script blocks.
pub fn clean_html(s: &str, aggressive: bool) -> String {
    // Pass 1: remove <script> and <style> blocks entirely
    let s = re_script_block().replace_all(s, "").into_owned();
    let s = re_style_block().replace_all(&s, "").into_owned();

    // Pass 2: remove HTML comments
    let s = re_html_comment().replace_all(&s, "").into_owned();

    // Pass 3: remove <head>...</head> block (metadata, not content)
    let s = re_head_block().replace_all(&s, "").into_owned();

    // Pass 4: remove <svg>...</svg> blocks (noise)
    let s = re_svg_block().replace_all(&s, "").into_owned();

    // Pass 5: remove <noscript>...</noscript> blocks
    let s = re_noscript_block().replace_all(&s, "").into_owned();

    // Pass 6: extract text content from tags
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

        if aggressive {
            // Aggressive: strip ALL tags and extract pure text
            let text = re_html_tag().replace_all(trimmed, " ").trim().to_string();
            if !text.is_empty() {
                let text = decode_entities(&text);
                out.push(text);
            }
        } else {
            // Non-aggressive: strip boilerplate tags (div, span, section, nav, footer, header,
            // aside, main, article, figure, figcaption, picture, source, meta, link, br, hr,
            // input, button, form, label, fieldset, legend, template, slot, iframe, embed, object)
            // but KEEP semantic/content tags (p, h1-h6, a, ul, ol, li, table, tr, td, th, pre, code, img)
            let cleaned = re_boilerplate_tags().replace_all(trimmed, "").to_string();
            // Also strip all attributes from remaining tags
            let cleaned = re_html_attrs().replace_all(&cleaned, "").to_string();
            let cleaned = re_all_attrs().replace_all(&cleaned, |caps: &regex::Captures| {
                // Keep the tag name, strip attributes
                let tag_start = &caps[1];
                let tag_end = &caps[2];
                format!("{tag_start}{tag_end}")
            }).to_string();
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
    // Remove class="...", style="...", id="...", data-*, aria-*, role attributes
    R.get_or_init(|| {
        Regex::new(r#"\s+(?:class|style|id|data-[\w-]+|aria-[\w-]+|role|tabindex|onclick|onload|onchange|onsubmit)\s*=\s*"[^"]*""#).unwrap()
    })
}

fn re_head_block() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?si)<head[^>]*>.*?</head>").unwrap())
}

fn re_svg_block() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?si)<svg[^>]*>.*?</svg>").unwrap())
}

fn re_noscript_block() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?si)<noscript[^>]*>.*?</noscript>").unwrap())
}

fn re_boilerplate_tags() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Strip open and close tags for layout/boilerplate elements
    R.get_or_init(|| {
        Regex::new(r"(?i)</?(?:div|span|section|nav|footer|header|aside|main|article|figure|figcaption|picture|source|meta|link|br|hr|input|button|form|label|fieldset|legend|template|slot|iframe|embed|object|details|summary)(?:\s[^>]*)?>").unwrap()
    })
}

fn re_all_attrs() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Capture tag opening + attributes + closing, to strip attributes from remaining tags
    R.get_or_init(|| {
        Regex::new(r"(<[a-zA-Z][a-zA-Z0-9]*)\s+[^>]*(>)").unwrap()
    })
}
