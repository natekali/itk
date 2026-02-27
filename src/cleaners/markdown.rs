use regex::Regex;
use std::sync::OnceLock;
use super::collapse_blank_lines;

const NOISE_SECTIONS: &[&str] = &[
    "installation",
    "getting started",
    "prerequisites",
    "requirements",
    "contributing",
    "contributors",
    "code of conduct",
    "license",
    "changelog",
    "releases",
    "roadmap",
    "acknowledgements",
    "acknowledgments",
    "credits",
    "sponsor",
];

pub fn clean_markdown(s: &str, aggressive: bool) -> String {
    // Pass 1: remove HTML comments <!-- ... -->
    let s = re_html_comment()
        .replace_all(s, "")
        .into_owned();

    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut skip_section = false;
    let mut skip_section_level = 0usize;

    for line in &lines {
        let trimmed = line.trim();

        // Count leading #s for heading level
        let heading_level = if trimmed.starts_with('#') {
            trimmed.chars().take_while(|c| *c == '#').count()
        } else {
            0
        };

        // End of a skipped section: new heading at same or higher level
        if skip_section && heading_level > 0 && heading_level <= skip_section_level {
            skip_section = false;
        }

        if skip_section {
            continue;
        }

        // Aggressive: skip known noise sections
        if aggressive && heading_level >= 2 {
            let heading_text = trimmed
                .trim_start_matches('#')
                .trim()
                .to_lowercase();
            if NOISE_SECTIONS.iter().any(|s| heading_text.starts_with(s)) {
                skip_section = true;
                skip_section_level = heading_level;
                continue;
            }
        }

        // Skip badge lines: standalone lines that are only image/badge markdown
        // Covers: ![alt](url), [![alt](img)](link), [![alt](img)](link)
        if is_badge_line(trimmed) {
            continue;
        }

        // Convert H3+ headings to bold bullet points
        if heading_level >= 3 {
            let heading_text = trimmed.trim_start_matches('#').trim();
            out.push(format!("- **{heading_text}**"));
            continue;
        }

        out.push(line.to_string());
    }

    // Collapse excessive blank lines
    collapse_blank_lines(&out.join("\n"), 1)
}

fn is_badge_line(line: &str) -> bool {
    // A badge line is one where the entire trimmed content consists of
    // markdown image/link patterns. Heuristic: line contains "![" or starts with
    // "[![" and has no plain text content outside of markdown link syntax.
    // Patterns:
    //   ![alt](url)
    //   [![alt](img_url)](link_url)
    //   Multiple badges on one line
    // Note: strip_ansi_escapes may convert [![ to \![, so check both forms.
    if !line.contains("![") && !line.contains("\\![") {
        return false;
    }
    // Normalise escaped bangs from ANSI stripping before matching
    let normalised = line.replace("\\!", "!");
    // Remove all markdown image-link patterns and check nothing substantive remains
    let cleaned = re_badge_pattern()
        .replace_all(&normalised, "")
        .trim()
        .to_string();
    cleaned.is_empty()
}

fn re_badge_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Matches: [![alt](img)](link) or ![alt](url)
    R.get_or_init(|| {
        Regex::new(r#"\[?!\[[^\]]*\]\([^)]*\)\]?(?:\([^)]*\))?"#).unwrap()
    })
}

fn re_html_comment() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").unwrap())
}
