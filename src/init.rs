use crate::style;
use std::fs;
use std::path::{Path, PathBuf};

// ── Public API ──────────────────────────────────────────────────────────────

pub fn run(global: bool, show: bool, uninstall: bool) {
    if show {
        show_status(global);
        return;
    }
    if uninstall {
        do_uninstall(global);
        return;
    }
    do_install(global);
}

// ── Install ─────────────────────────────────────────────────────────────────

fn do_install(global: bool) {
    let (hooks_dir, settings_path, itk_md_path) = resolve_paths(global);

    // 1. Create hooks directory
    if let Err(e) = fs::create_dir_all(&hooks_dir) {
        eprintln!("{} {} failed to create hooks directory: {e}", style::dim("itk:"), style::error("error:"));
        return;
    }

    // 2. Write hook script
    let hook_path = write_hook_script(&hooks_dir);
    let hook_path = match hook_path {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} {} failed to write hook script: {e}", style::dim("itk:"), style::error("error:"));
            return;
        }
    };

    // 3. Patch settings.json
    if let Err(e) = patch_settings(&settings_path, &hook_path, global) {
        eprintln!("{} {} failed to patch settings.json: {e}", style::dim("itk:"), style::error("error:"));
        eprintln!("  You can manually add the hook -- run {} for the JSON snippet.", style::info("itk init --show"));
        return;
    }

    // 4. Write ITK.md (global only)
    if global {
        if let Err(e) = write_itk_md(&itk_md_path) {
            eprintln!("{} {} could not write ITK.md: {e}", style::dim("itk:"), style::warning("warning:"));
        }
    }

    eprintln!("{} {}", style::dim("itk:"), style::success("hook installed successfully!"));
    eprintln!("  Hook script: {}", style::dim(&hook_path.display().to_string()));
    eprintln!("  Settings:    {}", style::dim(&settings_path.display().to_string()));
    if global {
        eprintln!("  ITK.md:      {}", style::dim(&itk_md_path.display().to_string()));
    }
    eprintln!();
    eprintln!("  Restart Claude Code for changes to take effect.");
    eprintln!("  Verify with: {}", style::info("itk init --show"));
}

// ── Uninstall ───────────────────────────────────────────────────────────────

fn do_uninstall(global: bool) {
    let (hooks_dir, settings_path, itk_md_path) = resolve_paths(global);

    // Remove hook script
    let hook_script = if cfg!(windows) {
        hooks_dir.join("itk-clean.ps1")
    } else {
        hooks_dir.join("itk-clean.sh")
    };
    if hook_script.exists() {
        let _ = fs::remove_file(&hook_script);
        eprintln!("{} removed {}", style::dim("itk:"), style::dim(&hook_script.display().to_string()));
    }

    // Remove ITK.md
    if itk_md_path.exists() {
        let _ = fs::remove_file(&itk_md_path);
        eprintln!("{} removed {}", style::dim("itk:"), style::dim(&itk_md_path.display().to_string()));
    }

    // Remove hook entry from settings.json
    if settings_path.exists() {
        if let Ok(content) = fs::read_to_string(&settings_path) {
            if content.contains("itk-clean") {
                let cleaned = remove_itk_from_settings(&content);
                let _ = fs::write(&settings_path, cleaned);
                eprintln!("{} removed hook from {}", style::dim("itk:"), style::dim(&settings_path.display().to_string()));
            }
        }
    }

    eprintln!("{} {}", style::dim("itk:"), style::success("uninstall complete. Restart Claude Code."));
}

// ── Show status ─────────────────────────────────────────────────────────────

fn show_status(global: bool) {
    let (hooks_dir, settings_path, itk_md_path) = resolve_paths(global);
    let scope = if global { "Global" } else { "Project" };

    let hook_script = if cfg!(windows) {
        hooks_dir.join("itk-clean.ps1")
    } else {
        hooks_dir.join("itk-clean.sh")
    };

    eprintln!("{}", style::header(&format!("ITK Hook Status ({scope})")));
    eprintln!("{}", style::dim("────────────────────────────────"));

    // Hook script
    if hook_script.exists() {
        eprintln!("  Hook script:  {} {}", style::dim(&hook_script.display().to_string()), style::success("(installed)"));
    } else {
        eprintln!("  Hook script:  {} {}", style::dim(&hook_script.display().to_string()), style::warning("(not found)"));
    }

    // Settings.json
    if settings_path.exists() {
        if let Ok(content) = fs::read_to_string(&settings_path) {
            if content.contains("itk-clean") {
                eprintln!("  Settings.json: {} {}", style::dim(&settings_path.display().to_string()), style::success("(hook registered)"));
            } else {
                eprintln!("  Settings.json: {} {}", style::dim(&settings_path.display().to_string()), style::error("(hook NOT registered)"));
            }
        }
    } else {
        eprintln!("  Settings.json: {} {}", style::dim(&settings_path.display().to_string()), style::warning("(not found)"));
    }

    // ITK.md
    if global {
        if itk_md_path.exists() {
            eprintln!("  ITK.md:       {} {}", style::dim(&itk_md_path.display().to_string()), style::success("(present)"));
        } else {
            eprintln!("  ITK.md:       {} {}", style::dim(&itk_md_path.display().to_string()), style::warning("(not found)"));
        }
    }

    // Show the JSON snippet for manual patching
    let hook_ref = hook_path_for_settings(&hook_script, global);
    eprintln!();
    eprintln!("Manual settings.json snippet:");
    eprintln!("```json");
    eprintln!("{}", settings_json_snippet(&hook_ref));
    eprintln!("```");
}

// ── Path resolution ─────────────────────────────────────────────────────────

fn resolve_paths(global: bool) -> (PathBuf, PathBuf, PathBuf) {
    if global {
        let claude_dir = claude_global_dir();
        let hooks_dir = claude_dir.join("hooks");
        let settings_path = claude_dir.join("settings.json");
        let itk_md_path = claude_dir.join("ITK.md");
        (hooks_dir, settings_path, itk_md_path)
    } else {
        let project_dir = PathBuf::from(".claude");
        let hooks_dir = project_dir.join("hooks");
        let settings_path = project_dir.join("settings.json");
        let itk_md_path = project_dir.join("ITK.md");
        (hooks_dir, settings_path, itk_md_path)
    }
}

fn claude_global_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".claude")
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile).join(".claude")
    } else {
        PathBuf::from(".claude")
    }
}

// ── Hook script generation ──────────────────────────────────────────────────

fn write_hook_script(hooks_dir: &Path) -> Result<PathBuf, std::io::Error> {
    if cfg!(windows) {
        let path = hooks_dir.join("itk-clean.ps1");
        fs::write(&path, HOOK_SCRIPT_WINDOWS)?;
        Ok(path)
    } else {
        let path = hooks_dir.join("itk-clean.sh");
        fs::write(&path, HOOK_SCRIPT_UNIX)?;
        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&path, perms)?;
        }
        Ok(path)
    }
}

const HOOK_SCRIPT_UNIX: &str = r#"#!/bin/bash
# ITK (Input Token Killer) — Claude Code UserPromptSubmit hook
# Automatically cleans large pasted content before Claude processes it.
# Installed by: itk init

set -e

# Read JSON input from stdin
INPUT=$(cat)

# Extract the user's prompt
PROMPT=$(echo "$INPUT" | jq -r '.prompt // empty' 2>/dev/null)

# Skip if prompt is too short (< 500 chars) — not worth optimizing
if [ ${#PROMPT} -lt 500 ]; then
  exit 0
fi

# Check if itk is available
if ! command -v itk &>/dev/null; then
  exit 0
fi

# Run ITK on the prompt content (compact mode, no frame for the hook context)
CLEANED=$(echo "$PROMPT" | itk --compact --no-frame --stats 2>/tmp/itk-hook-stats.txt || echo "")

# Read stats from stderr
STATS=$(cat /tmp/itk-hook-stats.txt 2>/dev/null | head -1 || echo "")
rm -f /tmp/itk-hook-stats.txt

# If ITK produced no output or content is the same, skip
if [ -z "$CLEANED" ] || [ "$CLEANED" = "$PROMPT" ]; then
  exit 0
fi

# Check if there were actual savings (parse stats line)
if echo "$STATS" | grep -q "no change"; then
  exit 0
fi

# Return the cleaned content as additional context
jq -n --arg ctx "$CLEANED" --arg stats "$STATS" '{
  hookSpecificOutput: {
    hookEventName: "UserPromptSubmit",
    additionalContext: ("[ITK optimized] " + $stats + "\n" + $ctx)
  }
}'
"#;

const HOOK_SCRIPT_WINDOWS: &str = r#"# ITK (Input Token Killer) — Claude Code UserPromptSubmit hook
# Automatically cleans large pasted content before Claude processes it.
# Installed by: itk init

$ErrorActionPreference = "Stop"

# Read JSON input from stdin
$input = [Console]::In.ReadToEnd()

# Parse JSON to get the prompt
try {
    $json = $input | ConvertFrom-Json
    $prompt = $json.prompt
} catch {
    exit 0
}

# Skip if prompt is too short
if (-not $prompt -or $prompt.Length -lt 500) {
    exit 0
}

# Check if itk is available
$itkPath = Get-Command itk -ErrorAction SilentlyContinue
if (-not $itkPath) {
    exit 0
}

# Run ITK on the prompt content
try {
    $cleaned = $prompt | itk --compact --no-frame 2>$null
} catch {
    exit 0
}

# If no output or same content, skip
if (-not $cleaned -or $cleaned -eq $prompt) {
    exit 0
}

# Return cleaned content as additional context
$result = @{
    hookSpecificOutput = @{
        hookEventName = "UserPromptSubmit"
        additionalContext = "[ITK optimized]`n$cleaned"
    }
} | ConvertTo-Json -Depth 4

Write-Output $result
"#;

// ── Settings.json patching ──────────────────────────────────────────────────

fn hook_path_for_settings(hook_path: &Path, global: bool) -> String {
    if global {
        // Use the actual path for global hooks
        hook_path.display().to_string().replace('\\', "/")
    } else {
        // Use $CLAUDE_PROJECT_DIR for project-local hooks
        let filename = hook_path.file_name().unwrap_or_default().to_string_lossy();
        format!("\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/{filename}")
    }
}

fn settings_json_snippet(hook_ref: &str) -> String {
    format!(
        r#"{{
  "hooks": {{
    "UserPromptSubmit": [
      {{
        "hooks": [
          {{
            "type": "command",
            "command": "{hook_ref}",
            "statusMessage": "ITK: optimizing input tokens..."
          }}
        ]
      }}
    ]
  }}
}}"#
    )
}

fn patch_settings(settings_path: &Path, hook_path: &Path, global: bool) -> Result<(), String> {
    let hook_ref = hook_path_for_settings(hook_path, global);

    if settings_path.exists() {
        let content = fs::read_to_string(settings_path)
            .map_err(|e| format!("could not read settings.json: {e}"))?;

        // Already has ITK hook?
        if content.contains("itk-clean") {
            eprintln!("{} hook already registered in {}", style::dim("itk:"), style::dim(&settings_path.display().to_string()));
            return Ok(());
        }

        // Try to merge into existing hooks
        let patched = merge_hook_into_settings(&content, &hook_ref);
        // Backup existing file
        let backup_path = settings_path.with_extension("json.bak");
        let _ = fs::copy(settings_path, &backup_path);
        fs::write(settings_path, patched)
            .map_err(|e| format!("could not write settings.json: {e}"))?;
        eprintln!("{} backup created at {}", style::dim("itk:"), style::dim(&backup_path.display().to_string()));
    } else {
        // Create new settings.json
        if let Some(parent) = settings_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let snippet = settings_json_snippet(&hook_ref);
        fs::write(settings_path, &snippet)
            .map_err(|e| format!("could not create settings.json: {e}"))?;
    }

    Ok(())
}

/// Merge ITK hook entry into existing settings.json content.
/// Uses simple string manipulation to avoid pulling in a full JSON library for manipulation.
fn merge_hook_into_settings(content: &str, hook_ref: &str) -> String {
    let hook_entry = format!(
        r#"{{
        "hooks": [
          {{
            "type": "command",
            "command": "{hook_ref}",
            "statusMessage": "ITK: optimizing input tokens..."
          }}
        ]
      }}"#
    );

    // Case 1: "UserPromptSubmit" already exists — append to its array
    if content.contains("\"UserPromptSubmit\"") {
        // Find the UserPromptSubmit array and append our hook
        if let Some(pos) = content.find("\"UserPromptSubmit\"") {
            // Find the opening [ of the array
            if let Some(bracket_pos) = content[pos..].find('[') {
                let insert_pos = pos + bracket_pos + 1;
                let mut patched = String::with_capacity(content.len() + hook_entry.len() + 10);
                patched.push_str(&content[..insert_pos]);
                patched.push('\n');
                patched.push_str("      ");
                patched.push_str(&hook_entry);
                patched.push(',');
                patched.push_str(&content[insert_pos..]);
                return patched;
            }
        }
    }

    // Case 2: "hooks" key exists but no UserPromptSubmit
    if content.contains("\"hooks\"") {
        if let Some(pos) = content.find("\"hooks\"") {
            // Find the opening { of the hooks object
            if let Some(brace_pos) = content[pos..].find('{') {
                let insert_pos = pos + brace_pos + 1;
                let entry = format!(
                    "\n    \"UserPromptSubmit\": [\n      {hook_entry}\n    ],"
                );
                let mut patched = String::with_capacity(content.len() + entry.len());
                patched.push_str(&content[..insert_pos]);
                patched.push_str(&entry);
                patched.push_str(&content[insert_pos..]);
                return patched;
            }
        }
    }

    // Case 3: No hooks at all — find the top-level object and add hooks
    if let Some(pos) = content.find('{') {
        let hooks_block = format!(
            "\n  \"hooks\": {{\n    \"UserPromptSubmit\": [\n      {hook_entry}\n    ]\n  }},"
        );
        let mut patched = String::with_capacity(content.len() + hooks_block.len());
        patched.push_str(&content[..pos + 1]);
        patched.push_str(&hooks_block);
        patched.push_str(&content[pos + 1..]);
        return patched;
    }

    // Fallback: return the full snippet
    settings_json_snippet(hook_ref)
}

/// Remove ITK hook entries from settings.json content.
fn remove_itk_from_settings(content: &str) -> String {
    // Simple approach: remove lines containing "itk-clean" and surrounding JSON structure
    // For a more robust solution, we'd parse JSON, but this is good enough for cleanup
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut skip_block = false;
    let mut brace_depth: i32 = 0;

    for line in &lines {
        if line.contains("itk-clean") || line.contains("ITK: optimizing") {
            // Start skipping — find the enclosing object
            skip_block = true;
            brace_depth = 0;
            continue;
        }
        if skip_block {
            for ch in line.chars() {
                match ch {
                    '{' | '[' => brace_depth += 1,
                    '}' | ']' => brace_depth -= 1,
                    _ => {}
                }
            }
            if brace_depth <= 0 {
                skip_block = false;
            }
            continue;
        }
        result.push(*line);
    }

    result.join("\n")
}

// ── ITK.md ──────────────────────────────────────────────────────────────────

fn write_itk_md(path: &Path) -> Result<(), std::io::Error> {
    fs::write(path, ITK_MD_CONTENT)
}

const ITK_MD_CONTENT: &str = r#"# ITK — Input Token Killer

ITK automatically optimizes input tokens via Claude Code hook.

- Large pasted content (>500 chars) is auto-cleaned before processing
- Stack traces, JSON, YAML, logs, diffs, code are detected and compressed
- Use `itk gain` to see token savings
- Use `itk init --show` to check hook status
- Use `itk discover` to find missed optimization opportunities
"#;
