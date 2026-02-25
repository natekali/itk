# ITK — Input Token Killer

> Compress clipboard and piped content before pasting into LLMs.
> Stack traces, diffs, logs, JSON — cleaned and token-optimised in <100ms.

**RTK for output. ITK for input. The ultimate LLM dev pair.**

## Install

**Linux / macOS:**
```bash
curl -fsSL https://itk-ai.app/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://itk-ai.app/install.ps1 | iex
```

**From source:**
```bash
cargo install --path .
```

---

## Usage

### Clipboard mode (most common)
Copy any messy content, run `itk`, paste the cleaned result.

```
itk
```

### Pipe mode
```bash
cat error.log | itk
git diff | itk --diff
curl -s api.example.com/v1/data | itk
```

### All options
```
itk [OPTIONS] [COMMAND]

COMMANDS:
  gain [--history]   Token savings dashboard

OPTIONS:
  -s, --summary           Add a 1-2 line summary header
      --aggressive        Collapse repeated frames, truncate deep traces
      --diff              Optimise for git diff / patch format
      --prompt <TYPE>     Wrap in prompt template: fix|explain|refactor|review|debug
      --stats             Print inline token savings as a header comment
  -h, --help
  -V, --version
```

### Examples

```bash
# Clean whatever is in your clipboard
itk

# Add summary + wrap in a fix-request prompt
itk --summary --prompt fix

# Compact a git diff with inline savings stats
git diff main...HEAD | itk --diff --stats

# Collapse a 200-frame Rust backtrace aggressively
itk --aggressive

# See your token savings dashboard
itk gain

# See per-run history
itk gain --history
```

---

## What gets cleaned

| Content | What ITK does |
|---|---|
| Python / JS / Rust / Go / Java stack traces | Extract root cause, collapse internal frames, truncate with `... [N frames truncated]` |
| Git diff / patch | Drop context lines beyond 2, keep all +/- lines, emit omission markers |
| Log files | Strip ANSI codes, strip timestamps, deduplicate identical lines, remove progress bars |
| JSON | Parse, pretty-print with 2-space indent, summarise large primitive arrays |
| YAML | Strip comments, collapse blank lines, normalise indentation |
| Code | Wrap in markdown fences with language tag, collapse blank runs |
| Plain text | Strip ANSI, trim trailing whitespace, collapse blank lines |

Auto-detection scans the first 4 KB — no config needed.

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

  By content type (all time):
  Type                       Runs  Avg savings
  ──────────────────────────────────────────────
  trace/Rust                   12       71.2%
  GitDiff                      10       58.4%
  LogFile                       9       81.0%
  Json                          8       52.1%
  Code                          5       41.3%
  PlainText                     3       28.7%
```

---

## How it works

1. **Detect** — scan first 4 KB against compiled regex patterns to classify content type
2. **Clean** — route to the type-specific cleaner; `--aggressive` enables deeper compression
3. **Write** — pipe mode: stdout; clipboard mode: overwrite clipboard, print savings to stderr
4. **Record** — append to SQLite history for `itk gain` (best-effort, never blocks)

---

## Design principles

- **Never panic** — `catch_unwind` wraps the clean path; any failure returns input unchanged
- **Pipeline-safe** — stdout is pure signal; all diagnostics go to stderr
- **Zero config** — no `~/.config/itk/` needed; history DB created lazily
- **Idempotent** — running `itk` twice on already-cleaned content is safe
- **Sub-100ms** — no network, no LLM, compiled regexes via `OnceLock`

---

## License

MIT — see [LICENSE](LICENSE)
