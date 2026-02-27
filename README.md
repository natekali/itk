# ITK — Input Token Killer

> Frame, compress, and prompt-wrap content before pasting into LLMs.
> Auto-detects stack traces, diffs, logs, JSON, YAML, code, build output, and markdown.

**RTK kills noise in output. ITK frames signal in input.**

## Install

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/natekali/itk/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/natekali/itk/main/install.ps1 | iex
```

**From source:**
```bash
cargo install --path .
```

**Update:**
```bash
itk update
```

---

## How it works

```
Input (clipboard or stdin)
  -> Detect    content type via regex heuristics
  -> Clean     type-specific compression
  -> Frame     [context header] with annotations
  -> Prompt    optional --prompt template wrapping
  -> Output    clipboard or stdout
```

Every invocation adds a lightweight context header that gives LLMs instant orientation — even when compression is zero, framing improves LLM response quality.

---

## Usage

### Clipboard mode (default)
Copy content, run `itk`, paste the framed result:
```
itk
```

### Pipe mode
```bash
cat error.log | itk
git diff | itk --diff
curl -s api.example.com/data | itk
```

### All options
```
itk [OPTIONS] [COMMAND]

COMMANDS:
  gain [--history]   Token savings dashboard
  update             Update itk to the latest release

OPTIONS:
      --no-frame         Disable context framing (raw cleaned output)
  -c, --compact          Safe compression: string truncation, number rounding
      --aggressive       Deep compression: strip metadata, remove defaults
      --diff             Optimise for git diff / patch format
      --type <TYPE>      Force content type: diff, log, json, yaml, trace,
                         rust, python, js, ts, go, java, markdown, build
      --prompt <TYPE>    Wrap in prompt template (see Prompt Templates below)
      --focus <TEXT>     Direct LLM attention to a specific area
      --stats            Print token savings as a header comment
  -h, --help
  -V, --version
```

---

## Context framing

Every `itk` invocation wraps output with a `[type | lines | annotations]` header. This costs ~10-30 tokens but gives LLMs instant context about what they're looking at.

**Stack trace:**
```
[trace/python | 23 lines | 15 frames | KeyError: 'user_id']
Traceback (most recent call last):
  ...
```

**Kubernetes YAML:**
```
[yaml | 156 lines | Kubernetes Deployment | 3 container(s) | resource limits set | health probes configured]
apiVersion: apps/v1
kind: Deployment
...
```

**JSON error response:**
```
[json | 42 lines | 8 top-level keys | error response]
{ "error": { ... } }
```

**TypeScript code:**
```
[code/typescript | 234 lines | 5 exported | 12 functions | 3 types/classes | contains tests]
export function Dashboard() { ... }
```

**Git diff:**
```
[git-diff | 89 lines | 3 file(s) | +45/-12 | includes renames]
diff --git a/src/main.rs b/src/main.rs
...
```

Disable with `--no-frame` for raw output.

---

## What gets cleaned

| Content | Default | `--compact` / `--aggressive` |
|---|---|---|
| Stack traces (Python/JS/Rust/Go/Java) | Collapse internal frames, extract root cause | Truncate deep traces, remove metadata |
| Git diff / patch | Drop excess context lines, keep +/- lines | Deeper context reduction |
| Log files | Strip ANSI/timestamps, deduplicate lines | Remove progress bars, collapse repeated patterns |
| JSON | Pretty-print, summarise arrays | Truncate long strings, round floats, strip metadata keys |
| YAML | Strip comments, collapse blanks | Remove `status:` sections, strip defaults |
| Code | Collapse blank runs, language-fence | Strip doc comments, collapse imports, remove test modules |
| Build output (Cargo/TSC/ESLint) | Group errors by file, strip progress lines | Deeper deduplication |
| Markdown | Collapse blanks | Remove badges, installation/license sections, compact headings |
| Plain text | Strip ANSI, trim whitespace | Deduplicate repeated lines, remove ASCII borders |

Auto-detection scans the first 4 KB — no config needed. Override with `--type`.

---

## Prompt templates

Wrap content in research-backed prompt structures with `--prompt <template>`:

```bash
itk --prompt fix        # "Identify the root cause and provide a minimal fix"
itk --prompt explain    # "Explain what this code does / what caused this error"
itk --prompt refactor   # "Refactor for readability and maintainability"
itk --prompt review     # "Review for bugs, security, performance, anti-patterns"
itk --prompt debug      # "Help me debug this — identify the most likely cause"
itk --prompt test       # "Write unit tests for this code"
itk --prompt optimize   # "Identify performance bottlenecks and suggest optimizations"
itk --prompt convert    # "Convert to the most appropriate equivalent format"
```

Templates are **content-type-aware**:
- A `--prompt fix` on a stack trace produces a different prompt than on code
- Role prompting: *"You are a senior Rust developer..."* (10-15% accuracy improvement)
- Structured output: *"Respond with: 1. Root cause 2. Fix 3. Prevention"* (reduces hallucination)

Combine with `--focus` to direct attention:
```bash
itk --prompt review --focus "the auth middleware"
```

---

## Examples

```bash
# Clean whatever is in your clipboard (framing on by default)
itk

# Compact a JSON API response (truncate strings, round floats)
itk --compact

# Frame a Kubernetes manifest and ask for a review
itk --prompt review

# Compress a git diff with inline stats
git diff main...HEAD | itk --diff --stats

# Collapse a 200-frame Rust backtrace aggressively
itk --aggressive

# Wrap a stack trace in a fix-request prompt
itk --prompt fix

# Direct LLM focus to a specific area
itk --prompt review --focus "error handling in the checkout flow"

# Raw output without framing
itk --no-frame

# Force content type detection
cat output.txt | itk --type build

# See your token savings dashboard
itk gain

# See per-run history
itk gain --history

# Update to latest version
itk update
```

---

## Token savings dashboard

```
itk gain
```

```
┌───────────────────────────────────────────────────┐
│               ITK — Token Savings                  │
├────────────────┬──────────────────┬───────────────┤
│                │     Today        │   All Time    │
├────────────────┼──────────────────┼───────────────┤
│ Runs           │             14   │           47  │
│ Tokens in      │          23.1K   │        87.4K  │
│ Tokens out     │           8.4K   │        31.2K  │
│ Tokens saved   │          14.7K   │        56.2K  │
│ Avg savings    │          63.6%   │        64.3%  │
│ Est. cost saved│         $0.0735  │       $0.2810 │
└────────────────┴──────────────────┴───────────────┘
```

---

## Design principles

- **Frame first, compress second** — even 0% compression adds value through context framing
- **Never panic** — `catch_unwind` wraps the clean path; any failure returns input unchanged
- **Pipeline-safe** — stdout is pure signal; all diagnostics go to stderr
- **Zero config** — no config files needed; history DB created lazily
- **Idempotent** — running `itk` twice on already-cleaned content is safe
- **Sub-100ms** — no network, no LLM, compiled regexes via `OnceLock`

---

## License

MIT — see [LICENSE](LICENSE)
