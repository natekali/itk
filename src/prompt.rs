use crate::detect::ContentType;

/// Wrap `content` in an LLM-optimised prompt template.
/// Templates are content-type-aware with role prompting and structured output.
pub fn wrap(content: &str, prompt_type: &str, ct: &ContentType) -> String {
    let role = role_for(ct);
    let task = task_for(prompt_type, ct);
    let format = output_format_for(prompt_type, ct);

    let mut out = String::with_capacity(content.len() + 300);

    // Role (top of prompt — strongest position)
    out.push_str(role);
    out.push_str("\n\n");

    // Content
    out.push_str(content);
    out.push_str("\n\n");

    // Task instruction
    out.push_str(task);

    // Structured output format (end of prompt — second strongest position)
    if !format.is_empty() {
        out.push_str("\n\n");
        out.push_str(format);
    }

    out
}

/// Role prompt based on content type (research shows 10-15% accuracy improvement).
fn role_for(ct: &ContentType) -> &'static str {
    match ct {
        ContentType::StackTrace(_) => "You are a senior developer debugging a production issue.",
        ContentType::GitDiff => "You are a senior developer performing a code review.",
        ContentType::LogFile => "You are a senior SRE analyzing production logs.",
        ContentType::Json => "You are a senior backend developer analyzing API data.",
        ContentType::Yaml => "You are a senior DevOps engineer reviewing infrastructure config.",
        ContentType::Code(lang) => match lang.as_str() {
            "rust" => "You are a senior Rust developer.",
            "python" => "You are a senior Python developer.",
            "typescript" | "ts" => "You are a senior TypeScript developer.",
            "javascript" | "js" => "You are a senior JavaScript developer.",
            "go" => "You are a senior Go developer.",
            "java" => "You are a senior Java developer.",
            _ => "You are a senior software engineer.",
        },
        ContentType::BuildOutput(_) => "You are a senior developer diagnosing build failures.",
        ContentType::Markdown => "You are a senior technical writer reviewing documentation.",
        ContentType::PlainText => "You are a senior software engineer.",
    }
}

/// Task instruction tailored to prompt type and content type.
fn task_for(prompt_type: &str, ct: &ContentType) -> &'static str {
    match prompt_type.to_lowercase().as_str() {
        "fix" => match ct {
            ContentType::StackTrace(_) => "Identify the root cause of this error and provide a minimal fix.",
            ContentType::BuildOutput(_) => "Fix these build errors. Address each error with the minimal change needed.",
            ContentType::Code(_) => "Identify any bugs in this code and provide fixes.",
            ContentType::GitDiff => "Review this diff for bugs introduced by the changes.",
            _ => "Identify the issue and provide a minimal fix.",
        },
        "explain" => match ct {
            ContentType::StackTrace(_) => "Explain what caused this error and why it happened.",
            ContentType::GitDiff => "Explain what these changes do and their impact.",
            ContentType::Code(_) => "Explain what this code does, focusing on the core logic.",
            _ => "Explain this clearly and concisely.",
        },
        "refactor" => "Refactor this code to improve readability and maintainability. List key changes.",
        "review" => match ct {
            ContentType::Code(_) => "Review this code for bugs, security issues, performance problems, and anti-patterns.",
            ContentType::GitDiff => "Review this diff for correctness, security, and maintainability.",
            _ => "Review this for issues and improvements.",
        },
        "debug" => "Help me debug this. Identify the most likely cause and suggest a minimal fix.",
        "test" => match ct {
            ContentType::Code(lang) => match lang.as_str() {
                "rust" => "Write unit tests for this code using #[test] and assert macros.",
                "python" => "Write pytest tests for this code with clear test names.",
                "typescript" | "ts" | "javascript" | "js" => "Write tests for this code using the project's test framework.",
                _ => "Write comprehensive unit tests for this code.",
            },
            _ => "Write tests that verify the behavior described in this content.",
        },
        "optimize" => "Identify performance bottlenecks and suggest optimizations. Focus on measurable improvements, not micro-optimizations.",
        "convert" => "Convert this to the most appropriate equivalent format, preserving all semantics.",
        _ => "Review this and provide your analysis.",
    }
}

/// Structured output format instructions (reduces hallucination and rambling).
fn output_format_for(prompt_type: &str, ct: &ContentType) -> &'static str {
    match prompt_type.to_lowercase().as_str() {
        "fix" => match ct {
            ContentType::StackTrace(_) | ContentType::BuildOutput(_) =>
                "Respond with:\n1. **Root cause**: one sentence\n2. **Fix**: code or config change\n3. **Prevention**: how to avoid this",
            _ =>
                "Respond with:\n1. **Issue**: what's wrong\n2. **Fix**: the corrected code\n3. **Why**: brief explanation",
        },
        "review" =>
            "For each issue found, respond with:\n- **Severity**: Critical / Warning / Suggestion\n- **Location**: file and line\n- **Issue**: description\n- **Fix**: suggested change",
        "debug" =>
            "Respond with:\n1. **Most likely cause**: one sentence\n2. **Evidence**: what points to this\n3. **Fix**: minimal code change",
        "test" => "",  // Let the LLM choose test structure
        "optimize" =>
            "For each optimization:\n- **What**: the bottleneck\n- **Why**: impact\n- **How**: the change",
        "explain" => "",  // Free-form explanation is fine
        "refactor" =>
            "Show the refactored code, then list key changes and why.",
        _ => "",
    }
}
