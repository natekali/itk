# itk - Input Token Killer

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Frame, compress, and prompt-wrap content before pasting into LLMs.**

[GitHub](https://github.com/natekali/itk)

itk auto-detects what you're pasting — stack traces, JSON, YAML, diffs, logs, code, build output, markdown, HTML, SQL, CSV, Dockerfiles, .env files, Terraform — cleans it, frames it with context annotations, and optionally wraps it in research-backed prompt templates. Your LLM gets better input, you save tokens and money.

**RTK kills noise in output. ITK frames signal in input.**

## Why ITK

When you paste raw content into ChatGPT, Claude, or any LLM:
- The LLM wastes tokens figuring out **what it's looking at**
- Redundant lines (repeated errors, boilerplate) burn through your context window
- No structure = worse responses

ITK solves this in <100ms, offline, with zero config:

| What You Paste | Raw | With ITK | Savings | What ITK Does |
|---|---|---|---|---|
| Python stack trace | ~2,400 | ~600 | **-75%** | Collapses internal frames, extracts root error |
| JSON error response | ~3,200 | ~2,100 | **-37%** | Removes metadata keys, truncates strings |
| Kubernetes YAML | ~5,200 | ~5,200 | **0%*** | Frames with `[K8s Deployment \| 3 containers \| probes configured]` |
| TypeScript file | ~4,800 | ~4,300 | **-11%** | Collapses imports, strips doc comments |
| Git diff (3 files) | ~1,800 | ~800 | **-55%** | Drops excess context lines, keeps +/- |
| Log file (1K lines) | ~8,000 | ~1,600 | **-80%** | Deduplicates, strips timestamps/ANSI |
| Markdown README | ~2,400 | ~1,400 | **-43%** | Removes badges, license/install sections |
| .env file | ~500 | ~450 | **-10%** | **Masks secret values** (API keys, tokens, passwords) |
| CSV (10K rows) | ~80,000 | ~1,200 | **-98%** | Shows header + first 5 rows + summary |
| HTML page | ~12,000 | ~4,000 | **-67%** | Strips tags, scripts, styles, extracts text |

*\*Even at 0% compression, framing gives the LLM instant context about the content -- improving response quality without removing a single line.*

> Token estimates based on typical real-world inputs. Actual savings vary by content size and structure.

## How It Works

```
  Without ITK:

  +----------+  paste 5,200 tokens  +----------+
  |   You    | ------------------> |   LLM    |   "What is this YAML?"
  +----------+   raw, unstructured  +----------+   LLM guesses context
                                                   from scratch


  With ITK:

  +----------+  copy    +----------+  paste 2,100 tokens    +----------+
  |   You    | ------> |   ITK    | ---------------------> |   LLM    |
  +----------+         +----------+   framed + compressed   +----------+
                        <100ms                               LLM knows exactly
                        offline                              what it's looking at
                        zero config
```

Four stages, applied per content type:

1. **Detect** -- scan first 8 KB against compiled regex patterns to classify content
2. **Clean** -- route to type-specific cleaner (collapse frames, strip noise, deduplicate)
3. **Frame** -- wrap with `[type | lines | annotations]` context header
4. **Prompt** -- optionally wrap in a role-prompted template with structured output

---

## Installation

### Linux / macOS
```bash
curl -fsSL https://raw.githubusercontent.com/natekali/itk/main/install.sh | sh
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/natekali/itk/main/install.ps1 | iex
```

### From source
```bash
cargo install --path .
```

### Update
```bash
itk update
```

### Verify
```bash
itk --version    # Should show "itk 0.4.0"
```

## Quick Start

```bash
# 1. Copy a stack trace / JSON / YAML / whatever to clipboard
# 2. Run itk
itk

# 3. Paste into your LLM -- content is now cleaned, framed, and optimized
# That's it. No config, no flags needed.

# Or pipe directly:
cat error.log | itk
git diff | itk --diff
curl -s api.example.com/data | itk --compact

# Process files directly:
itk src/main.rs --prompt review
itk error.log --stats
itk config.yaml --compact
```

---

## Zero-Friction: Claude Code Integration

Install once, never think about it again. ITK automatically optimizes large prompts before they reach Claude:

```bash
itk init --global          # Install hook + ITK.md globally
itk init                   # Install for current project only
itk init --show            # Show current hook status
itk init --global --uninstall  # Remove everything
```

After `itk init`, every large prompt you submit is automatically cleaned and framed. The hook uses `additionalContext` -- your original prompt is preserved, the LLM gets both the raw and optimized versions.

---

## File Input Mode

Process files directly without clipboard gymnastics:

```bash
itk error.log                    # Clean file, write to clipboard
itk src/main.rs --prompt review  # Frame + prompt-wrap a source file
itk config.yaml --compact        # Compact a config file
cat a.log b.log | itk            # Pipe still works
```

---

## Preview Mode

See what ITK would do without modifying the clipboard:

```bash
itk --dry-run
```

```
+-- ITK Preview -------------------------------------------+
| Detected: trace/python (23 lines)
| Savings:  2,400 -> 600 tokens (-75%)
| Frame:    [trace/python | 5 frames | KeyError]
| Lines:    23 -> 18 (5 removed)
+----------------------------------------------------------+
[cleaned content written to stdout for inspection]
```

---

## Content Types (15 supported)

ITK auto-detects content type by scanning the first 8 KB. Override with `--type <TYPE>`.

| Type | Detection | Frame Annotation Example |
|---|---|---|
| **Stack traces** (Python/JS/Rust/Go/Java) | `Traceback`, `at Object.`, `panic:` | `[trace/python \| 23 lines \| 5 frames \| KeyError]` |
| **Git diffs** | `diff --git`, `@@`, `+/-` | `[git-diff \| 32 lines \| 2 file(s) \| +17/-0]` |
| **Log files** | Timestamps + ERROR/WARN/INFO | `[log \| 200 lines \| 4 error(s) \| 3 warning(s)]` |
| **JSON** | `{` / `[` + valid structure | `[json \| 42 lines \| 4 top-level keys \| error response]` |
| **YAML** | `key: value`, Kubernetes/Docker Compose | `[yaml \| 156 lines \| K8s Deployment \| 3 containers]` |
| **Code** (Rust/Python/TS/JS/Go/Java) | Language-specific syntax | `[code/typescript \| 234 lines \| 5 exported \| 12 functions]` |
| **Build output** (Cargo/TSC/ESLint) | `error[E`, `TS2`, `eslint` | `[build/cargo \| 45 lines \| 3 errors \| 1 warning]` |
| **Markdown** | `#`, `**`, `[text](url)` | `[markdown \| 120 lines \| 8 headings \| 3 code blocks]` |
| **HTML** | `<html`, `<!DOCTYPE`, `<div` | `[html \| 234 lines \| 3 forms \| 12 inputs]` |
| **SQL** | `SELECT`, `CREATE TABLE`, `INSERT` | `[sql \| 45 lines \| 3 queries \| JOIN detected]` |
| **CSV** | Comma-delimited header row | `[csv \| 10,000 rows \| 8 columns]` |
| **Dockerfile** | `FROM`, `RUN`, `COPY`, `EXPOSE` | `[dockerfile \| 34 lines \| 2 stages \| EXPOSE 3000]` |
| **.env** | `KEY=value` lines | `[env \| 12 vars \| 3 secrets masked]` |
| **Terraform** | `resource "`, `variable "` | `[terraform \| 89 lines \| 4 resources \| 2 modules]` |
| **Plain text** | Fallback | `[text \| 50 lines]` |

### .env Secret Masking

ITK automatically masks sensitive values in `.env` files to prevent accidental leaking into LLM context:

```
# Before
DATABASE_URL=postgres://admin:s3cret@db.example.com:5432/prod
API_KEY=sk-abc123xyz
JWT_SECRET=my-super-secret-key
APP_NAME=my-app

# After (itk)
DATABASE_URL=***
API_KEY=***
JWT_SECRET=***
APP_NAME=my-app
```

Detects 30+ secret patterns: `SECRET`, `KEY`, `TOKEN`, `PASSWORD`, `API_KEY`, `DATABASE_URL`, `PRIVATE`, `CREDENTIAL`, and more.

---

## Context Framing

Every `itk` invocation wraps output with a lightweight context header:

```
[type | line count | smart annotations extracted from content]
```

This costs ~10-30 tokens but gives LLMs instant orientation -- no more wasting tokens figuring out what they're looking at.

**Python stack trace:**
```
[trace/python | 23 lines | 15 frames | sqlalchemy.exc.NoResultFound: No row was found]
Traceback (most recent call last):
  File "/app/main.py", line 234, in handle_request
    result = process_order(request.data)
  ...
sqlalchemy.exc.NoResultFound: No row was found when one was required
```

**Kubernetes YAML:**
```
[yaml | 156 lines | Kubernetes Deployment | 3 container(s) | resource limits set | health probes configured]
apiVersion: apps/v1
kind: Deployment
...
```

**JSON API error:**
```
[json | 42 lines | 4 top-level keys | error response]
{
  "errors": [ ... ],
  "status": 422,
  ...
}
```

Disable framing with `--no-frame` for raw cleaned output.

---

## What Gets Cleaned

| Content | Default (clean + frame) | `--compact` | `--aggressive` |
|---|---|---|---|
| **Stack traces** (Python/JS/Rust/Go/Java) | Collapse internal frames, extract root cause | -- | Truncate deep traces |
| **Git diff / patch** | Drop excess context lines, keep all +/- | -- | Deeper context trimming |
| **Log files** | Strip ANSI, strip timestamps, deduplicate | -- | Remove progress bars |
| **JSON** | Pretty-print, summarise arrays, extract errors | Truncate long strings, round floats | Strip metadata keys (`_links`, `__typename`, etc.) |
| **YAML** | Strip comments, collapse blanks | -- | Remove `status:` sections, strip defaults |
| **Code** | Collapse blank runs, wrap in fenced block | -- | Strip doc comments, collapse imports, remove test modules |
| **Build output** (Cargo/TSC/ESLint) | Group errors by file | -- | Deeper deduplication |
| **Markdown** | Collapse blanks | -- | Remove badges, installation/license sections |
| **HTML** | Strip comments, remove script/style blocks | -- | Strip all tags, extract pure text content |
| **SQL** | Remove comments, normalize whitespace | Uppercase keywords | Collapse INSERT VALUES rows |
| **CSV** | Header + first 5 data rows + summary | -- | Header + first 3 rows, truncate long cells |
| **Dockerfile** | Collapse blanks | -- | Strip comments, collapse multi-line RUN |
| **.env** | Mask secret values (`API_KEY=***`) | -- | Also strip comments |
| **Terraform** | Strip comments | -- | Remove defaults, collapse description blocks |
| **Plain text** | Strip ANSI, trim whitespace | -- | Deduplicate repeated lines, remove ASCII borders |

Auto-detection scans the first 8 KB -- no config needed. Override with `--type`.

---

## Compression Levels

| Level | Flag | What it does | Best for |
|---|---|---|---|
| **Default** | *(none)* | Clean + Frame | Daily use -- safe, preserves everything |
| **Compact** | `-c` / `--compact` | + string truncation (>200 chars), float rounding (2dp) | Large JSON/YAML payloads |
| **Aggressive** | `--aggressive` | + metadata removal, section stripping, test module removal | Huge dumps, Kubernetes `kubectl get` output |

Levels are cumulative: `--aggressive` includes everything `--compact` does, plus more.

---

## Prompt Templates

Wrap content in research-backed prompt structures with `--prompt <template>`:

```bash
itk --prompt fix        # "Identify the root cause and provide a minimal fix"
itk --prompt explain    # "Explain what this code does / what caused this error"
itk --prompt refactor   # "Refactor for readability and maintainability"
itk --prompt review     # "Review for bugs, security, performance, anti-patterns"
itk --prompt debug      # "Help me debug this -- identify the most likely cause"
itk --prompt test       # "Write unit tests for this code"
itk --prompt optimize   # "Identify performance bottlenecks and suggest optimizations"
itk --prompt convert    # "Convert to the most appropriate equivalent format"
itk --prompt document   # "Generate documentation for all public items"
itk --prompt migrate    # "Migrate to the target framework/language"
itk --prompt security   # "Audit for security vulnerabilities"
```

### What makes these prompts better

Templates are **content-type-aware** -- a `--prompt fix` on a stack trace produces a completely different prompt than on code:

**Stack trace + `--prompt fix`:**
```
You are a senior developer debugging a production issue.

[trace/python | 6 lines | 2 frames | sqlalchemy.exc.NoResultFound: No row was found]
Traceback (most recent call last):
  File "/app/views.py", line 42, in get_user
    ...
sqlalchemy.exc.NoResultFound: No row was found for one()

Identify the root cause of this error and provide a minimal fix.

Respond with:
1. **Root cause**: one sentence
2. **Fix**: code or config change
3. **Prevention**: how to avoid this
```

**Why this works:**
- **Role prompting**: *"You are a senior {lang} developer..."* -- 10-15% accuracy improvement (research-backed)
- **Structured output format**: *"Respond with: 1. Root cause 2. Fix 3. Prevention"* -- reduces hallucination
- **Context framing**: LLM instantly knows it's a Python SQLAlchemy trace, not generic text

Combine with `--focus` to direct attention:
```bash
itk --prompt review --focus "the auth middleware"
# Adds [Focus: the auth middleware] to the frame
```

---

## Find Missed Savings

Discover how much you could have saved on recent Claude Code sessions:

```bash
itk discover                    # Current project, last 30 days
itk discover --all              # All Claude Code projects
itk discover --since 7          # Last 7 days
```

Example output:
```
ITK Discover -- Missed Savings
====================================================
Scanned: 89 sessions (last 30 days), 342 user messages with pasted content

CONTENT YOU COULD HAVE OPTIMIZED
----------------------------------------------------
Content Type          Count    Est. Tokens    Potential Savings
Stack traces             23       ~18.4K           ~13.8K (-75%)
JSON payloads            31       ~42.1K           ~15.6K (-37%)
Log files                 8       ~12.0K            ~9.6K (-80%)
Code files               19       ~28.5K            ~3.1K (-11%)
Git diffs                12        ~8.4K            ~4.6K (-55%)
----------------------------------------------------
Total: 93 messages -> ~46.7K tokens saveable
```

---

## Token Savings Dashboard

Track your savings over time:

```bash
itk gain                      # Today + all time overview
itk gain --since 7            # Today + last 7 days
itk gain --daily              # Day-by-day breakdown (last 30 days)
itk gain --daily --since 7    # Day-by-day for last 7 days
itk gain --history            # Per-run history (last 50 runs)
itk gain --format json        # Export all runs as JSON
itk gain --format csv         # Export all runs as CSV
itk gain --format json --since 7  # Export last 7 days as JSON
```

```
+-------------------------------------------------+
|               ITK -- Token Savings               |
+----------------+------------------+--------------+
|                |            Today |     All Time |
+----------------+------------------+--------------+
| Runs           |               14 |           47 |
| Tokens in      |           23.1K |        87.4K |
| Tokens out     |            8.4K |        31.2K |
| Tokens saved   |           14.7K |        56.2K |
| Avg savings    |           63.6% |        64.3% |
| Est. cost saved|          $0.0735 |      $0.2810 |
+----------------+------------------+--------------+

  By content type (all time):
  Type                       Runs  Avg savings
  ----------------------------------------------
  trace/rust                   12       71.2%
  git-diff                     10       58.4%
  log                           9       81.0%
  json                          8       52.1%
  code/typescript               5       41.3%
  text                          3       28.7%
```

---

## Config File

Optional per-project or global defaults via `.itk.json`:

```json
{
  "defaults": {
    "compact": true,
    "aggressive": false,
    "no_frame": false,
    "stats": true
  }
}
```

Search order: `./.itk.json` (project) > `~/.config/itk/config.json` (global). CLI flags always override config.

---

## Shell Completions

Generate completions for your shell:

```bash
itk completions bash > /etc/bash_completion.d/itk
itk completions zsh > ~/.zfunc/_itk
itk completions fish > ~/.config/fish/completions/itk.fish
itk completions powershell > itk.ps1
```

---

## All Options

```
itk [OPTIONS] [FILE] [COMMAND]

COMMANDS:
  gain                  Token savings dashboard
    --history           Show per-run history (last 50 runs)
    --daily             Show day-by-day breakdown
    --since <DAYS>      Only show data from the last N days
    --format <FORMAT>   Export format: json, csv
  discover              Find missed savings in Claude Code sessions
    --all               Scan all projects (not just current)
    --since <DAYS>      Only scan sessions from last N days (default: 30)
  init                  Install Claude Code hook for automatic optimization
    --global, -g        Install globally (~/.claude/)
    --show              Show current hook status
    --uninstall         Remove ITK hook and ITK.md
  completions <SHELL>   Generate shell completions (bash/zsh/fish/powershell)
  update                Update itk to the latest release

OPTIONS:
      --no-frame         Disable context framing (raw cleaned output)
  -c, --compact          Safe compression: string truncation, number rounding
      --aggressive       Deep compression: strip metadata, remove defaults
      --diff             Optimise for git diff / patch format
      --type <TYPE>      Force content type: diff, log, json, yaml, trace,
                         rust, python, js, ts, go, java, markdown, build,
                         html, sql, csv, dockerfile, env, terraform
      --prompt <TYPE>    Wrap in prompt template: fix|explain|refactor|review|
                         debug|test|optimize|convert|document|migrate|security
      --focus <TEXT>     Direct LLM attention to a specific area
      --stats            Print token savings as a header comment
      --dry-run          Preview without modifying clipboard
  -h, --help
  -V, --version
```

---

## ITK + RTK -- The Complete Pair

| | RTK | ITK |
|---|---|---|
| **Direction** | Output (command -> LLM) | Input (you -> LLM) |
| **How** | Proxy wrapping CLI commands | Clipboard/pipe/file processor |
| **When** | Automatic via hook | Automatic via `itk init`, or manual |
| **Savings** | 60-90% on command output | 10-80% on pasted content |
| **Unique value** | Filters noise from `git`, `cargo`, `ls` | Frames and prompt-wraps your content |

Use both together for maximum token efficiency:

```bash
# RTK handles output -- automatic via hook
rtk git diff          # LLM sees compact diff
rtk cargo test        # LLM sees failures only

# ITK handles input -- automatic via itk init, or manual
itk                   # Clean + frame clipboard
itk error.log         # Process a file directly
itk --prompt fix      # Wrap in fix-request prompt
```

---

## Design Principles

- **Frame first, compress second** -- even 0% compression adds value through context framing
- **Never panic** -- `catch_unwind` wraps the clean path; any failure returns input unchanged
- **Pipeline-safe** -- stdout is pure signal; all diagnostics go to stderr
- **Zero config** -- no config files needed; history DB created lazily
- **Idempotent** -- running `itk` twice on already-cleaned content is safe
- **Sub-100ms** -- no network, no LLM, compiled regexes via `OnceLock`
- **Secret-safe** -- .env files have sensitive values masked automatically

---

## Troubleshooting

### No clipboard access
**Problem**: `itk: clipboard not available` on headless/SSH/WSL systems

**Solution**: Use pipe mode or file mode instead:
```bash
cat error.log | itk > cleaned.txt
itk error.log > cleaned.txt
```

### Wrong content type detected
**Problem**: ITK misdetects your content (e.g., treats code as plain text)

**Solution**: Force the type:
```bash
itk --type rust
itk --type json
itk --type yaml
itk --type html
itk --type terraform
```

### Frame overhead on small inputs
**Problem**: The `[type | ...]` header adds tokens to very small inputs

**Solution**: Disable framing:
```bash
itk --no-frame
```

### Content unchanged
**Problem**: `itk` shows "no change" on some content types

**Expected**: Not all content can be compressed (clean YAML, minimal JSON). The context frame still adds value by giving the LLM instant orientation. Use `--compact` or `--aggressive` for deeper compression.

### Preview before committing
**Problem**: Want to see what ITK would do before modifying clipboard

**Solution**: Use dry-run mode:
```bash
itk --dry-run
```

---

## License

MIT -- see [LICENSE](LICENSE)

---

## Contributing

Contributions welcome! Open an issue or PR on [GitHub](https://github.com/natekali/itk).

Issues: [github.com/natekali/itk/issues](https://github.com/natekali/itk/issues)
