use crate::detect::ContentType;

/// Wrap `content` in an LLM-optimised prompt template.
/// `prompt_type`: fix | explain | refactor | review | debug
pub fn wrap(content: &str, prompt_type: &str, ct: &ContentType) -> String {
    let ctx = context_hint(ct);
    match prompt_type.to_lowercase().as_str() {
        "fix" => format!(
            "Please fix the following {ctx}.\n\
             Explain the root cause briefly, then provide the corrected code or solution.\n\n\
             {content}"
        ),
        "explain" => format!(
            "Please explain the following {ctx} clearly and concisely.\n\
             Focus on what it does, why it matters, and any important edge cases.\n\n\
             {content}"
        ),
        "refactor" => format!(
            "Please refactor the following {ctx} to improve readability, \
             performance, and maintainability.\n\
             List the key changes you made and why.\n\n\
             {content}"
        ),
        "review" => format!(
            "Please review the following {ctx}.\n\
             Point out any bugs, anti-patterns, security issues, or improvements.\n\n\
             {content}"
        ),
        "debug" => format!(
            "I am debugging the following {ctx}.\n\
             Help me identify the issue and suggest a minimal fix.\n\n\
             {content}"
        ),
        _ => format!(
            "Please review the following {ctx}:\n\n{content}"
        ),
    }
}

fn context_hint(ct: &ContentType) -> &'static str {
    match ct {
        ContentType::StackTrace(_) => "stack trace / error",
        ContentType::GitDiff => "git diff / patch",
        ContentType::LogFile => "log output",
        ContentType::Json => "JSON data",
        ContentType::Yaml => "YAML configuration",
        ContentType::Code(_) => "code",
        ContentType::BuildOutput(_) => "build output",
        ContentType::Markdown => "documentation",
        ContentType::PlainText => "text",
    }
}
