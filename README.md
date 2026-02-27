# itk - Input Token Killer

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Frame, compress, and prompt-wrap content before pasting into LLMs.**

[GitHub](https://github.com/natekali/itk)

itk auto-detects what you're pasting — stack traces, JSON, YAML, diffs, logs, code, build output, markdown — cleans it, frames it with context annotations, and optionally wraps it in research-backed prompt templates. Your LLM gets better input, you save tokens and money.

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

*\*Even at 0% compression, framing gives the LLM instant context about the content — improving response quality without removing a single line.*

> Token estimates based on typical real-world inputs. Actual savings vary by content size and structure.

## How It Works

```
  Without ITK:

  ┌──────────┐  paste 5,200 tokens  ┌──────────┐
  │   You    │ ───────────────────> │   LLM    │   "What is this YAML?"
  └──────────┘   raw, unstructured  └──────────┘   LLM guesses context
                                                   from scratch


  With ITK:

  ┌──────────┐  copy    ┌──────────┐  paste 2,100 tokens    ┌──────────┐
  │   You    │ ──────> │   ITK    │ ─────────────────────> │   LLM    │
  └──────────┘         └──────────┘   framed + compressed   └──────────┘
                        <100ms                               LLM knows exactly
                        offline                              what it's looking at
                        zero config
```

Four stages, applied per content type:

1. **Detect** — scan first 4 KB against compiled regex patterns to classify content
2. **Clean** — route to type-specific cleaner (collapse frames, strip noise, deduplicate)
3. **Frame** — wrap with `[type | lines | annotations]` context header
4. **Prompt** — optionally wrap in a role-prompted template with structured output

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
itk --version    # Should show "itk 0.3.0"
```

## Quick Start

```bash
# 1. Copy a stack trace / JSON / YAML / whatever to clipboard
# 2. Run itk
itk

# 3. Paste into your LLM — content is now cleaned, framed, and optimized
# That's it. No config, no flags needed.

# Or pipe directly:
cat error.log | itk
git diff | itk --diff
curl -s api.example.com/data | itk --compact
```

---

## Context Framing

Every `itk` invocation wraps output with a lightweight context header:

```
[type | line count | smart annotations extracted from content]
```

This costs ~10-30 tokens but gives LLMs instant orientation — no more wasting tokens figuring out what they're looking at.

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

**TypeScript code:**
```
[code/typescript | 234 lines | 5 exported | 12 functions | 3 types/classes]
export function Dashboard() { ... }
```

**Git diff:**
```
[git-diff | 32 lines | 2 file(s) | +17/-0]
diff --git a/src/main.rs b/src/main.rs
...
```

**Log file:**
```
[log | 200 lines | 4 error(s) | 3 warning(s)]
2024-01-15 10:30:11 ERROR Connection refused to database replica-2
  [... 2 identical lines suppressed]
...
```

**Docker Compose YAML:**
```
[yaml | 89 lines | Docker Compose | 4 service(s)]
```

**GitHub Actions YAML:**
```
[yaml | 112 lines | GitHub Actions workflow | 3 job(s)]
```

Disable framing with `--no-frame` for raw cleaned output.

---

## What Gets Cleaned

| Content | Default (clean + frame) | `--compact` | `--aggressive` |
|---|---|---|---|
| **Stack traces** (Python/JS/Rust/Go/Java) | Collapse internal frames, extract root cause | — | Truncate deep traces |
| **Git diff / patch** | Drop excess context lines, keep all +/- | — | Deeper context trimming |
| **Log files** | Strip ANSI, strip timestamps, deduplicate | — | Remove progress bars |
| **JSON** | Pretty-print, summarise arrays, extract errors | Truncate long strings, round floats | Strip metadata keys (`_links`, `__typename`, etc.) |
| **YAML** | Strip comments, collapse blanks | — | Remove `status:` sections, strip defaults |
| **Code** | Collapse blank runs, wrap in fenced block | — | Strip doc comments, collapse imports, remove test modules |
| **Build output** (Cargo/TSC/ESLint) | Group errors by file | — | Deeper deduplication |
| **Markdown** | Collapse blanks | — | Remove badges, installation/license sections |
| **Plain text** | Strip ANSI, trim whitespace | — | Deduplicate repeated lines, remove ASCII borders |

Auto-detection scans the first 4 KB — no config needed. Override with `--type`.

---

## Compression Levels

| Level | Flag | What it does | Best for |
|---|---|---|---|
| **Default** | *(none)* | Clean + Frame | Daily use — safe, preserves everything |
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
itk --prompt debug      # "Help me debug this — identify the most likely cause"
itk --prompt test       # "Write unit tests for this code"
itk --prompt optimize   # "Identify performance bottlenecks and suggest optimizations"
itk --prompt convert    # "Convert to the most appropriate equivalent format"
```

### What makes these prompts better

Templates are **content-type-aware** — a `--prompt fix` on a stack trace produces a completely different prompt than on code:

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
- **Role prompting**: *"You are a senior {lang} developer..."* — 10-15% accuracy improvement (research-backed)
- **Structured output format**: *"Respond with: 1. Root cause 2. Fix 3. Prevention"* — reduces hallucination
- **Context framing**: LLM instantly knows it's a Python SQLAlchemy trace, not generic text

Combine with `--focus` to direct attention:
```bash
itk --prompt review --focus "the auth middleware"
# Adds [Focus: the auth middleware] to the frame
```

---

## Examples — Standard vs ITK

### Stack trace

**Before** (raw paste, ~140 tokens):
```
Traceback (most recent call last):
  File "/app/main.py", line 234, in handle_request
    result = process_order(request.data)
  File "/app/services/orders.py", line 89, in process_order
    user = get_user(order["user_id"])
  File "/app/services/users.py", line 45, in get_user
    return db.session.query(User).filter_by(id=user_id).one()
  File "/venv/lib/sqlalchemy/orm/query.py", line 3423, in one
    raise NoResultFound("No row was found")
  File "/venv/lib/sqlalchemy/engine/result.py", line 560, in one_or_none
    return self._only_one_row(True, True, False)
  File "/venv/lib/sqlalchemy/engine/result.py", line 498, in _only_one_row
    raise NoResultFound("No row was found when one was required")
sqlalchemy.exc.NoResultFound: No row was found when one was required
```

**After** (`itk --aggressive`, ~107 tokens, -23%):
```
[trace/python | 13 lines | 5 frames | sqlalchemy.exc.NoResultFound: No row was found]
Traceback (most recent call last):
  File "/app/main.py", line 234, in handle_request
    result = process_order(request.data)
  File "/app/services/orders.py", line 89, in process_order
    user = get_user(order["user_id"])
  File "/app/services/users.py", line 45, in get_user
    return db.session.query(User).filter_by(id=user_id).one()
  File "/venv/lib/sqlalchemy/orm/query.py", line 3423, in one
    raise NoResultFound("No row was found")
  ... [frames truncated by itk]
sqlalchemy.exc.NoResultFound: No row was found when one was required
```

### Git diff

**Before** (raw, ~180 tokens):
```
diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,8 @@ fn main() {
     let config = Config::load();
     let db = Database::connect(&config.db_url);
     let server = Server::new(config.port);
+    let auth = AuthMiddleware::new(&config.jwt_secret);
+    server.use_middleware(auth);
     server.start();
 }
```

**After** (`itk --diff`, ~180 tokens, framed):
```
[git-diff | 32 lines | 2 file(s) | +17/-0]
diff --git a/src/main.rs b/src/main.rs
 ... [1 context lines omitted]
     let db = Database::connect(&config.db_url);
     let server = Server::new(config.port);
+    let auth = AuthMiddleware::new(&config.jwt_secret);
+    server.use_middleware(auth);
     server.start();
```

### Log file

**Before** (raw, 20 lines — imagine 200):
```
2024-01-15 10:30:11 ERROR Connection refused to database replica-2 at 10.0.1.5:5432
2024-01-15 10:30:11 ERROR Connection refused to database replica-2 at 10.0.1.5:5432
2024-01-15 10:30:11 ERROR Connection refused to database replica-2 at 10.0.1.5:5432
...
```

**After** (`itk --aggressive`, deduplicated + annotated):
```
[log | 19 lines | 2 error(s) | 3 warning(s)]
2024-01-15 10:30:11 ERROR Connection refused to database replica-2 at 10.0.1.5:5432
  [... 2 identical lines suppressed]
2024-01-15 10:30:12 WARN  Failover to replica-3 initiated
...
```

---

## All Options

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
      --prompt <TYPE>    Wrap in prompt template: fix|explain|refactor|review|
                         debug|test|optimize|convert
      --focus <TEXT>     Direct LLM attention to a specific area
      --stats            Print token savings as a header comment
  -h, --help
  -V, --version
```

---

## Token Savings Dashboard

Track your savings over time:

```bash
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
  trace/rust                   12       71.2%
  git-diff                     10       58.4%
  log                           9       81.0%
  json                          8       52.1%
  code/typescript               5       41.3%
  text                          3       28.7%
```

```bash
itk gain --history    # Per-run history (last 50 runs)
```

---

## ITK + RTK — The Complete Pair

| | RTK | ITK |
|---|---|---|
| **Direction** | Output (command → LLM) | Input (you → LLM) |
| **How** | Proxy wrapping CLI commands | Clipboard/pipe processor |
| **When** | Automatic via hook | Manual: copy → `itk` → paste |
| **Savings** | 60-90% on command output | 10-80% on pasted content |
| **Unique value** | Filters noise from `git`, `cargo`, `ls` | Frames and prompt-wraps your content |

Use both together for maximum token efficiency:

```bash
# RTK handles output — automatic via hook
rtk git diff          # LLM sees compact diff
rtk cargo test        # LLM sees failures only

# ITK handles input — you paste smarter
itk                   # Clean + frame clipboard
itk --prompt fix      # Wrap in fix-request prompt
```

---

## Design Principles

- **Frame first, compress second** — even 0% compression adds value through context framing
- **Never panic** — `catch_unwind` wraps the clean path; any failure returns input unchanged
- **Pipeline-safe** — stdout is pure signal; all diagnostics go to stderr
- **Zero config** — no config files needed; history DB created lazily
- **Idempotent** — running `itk` twice on already-cleaned content is safe
- **Sub-100ms** — no network, no LLM, compiled regexes via `OnceLock`

---

## Troubleshooting

### No clipboard access
**Problem**: `itk: clipboard not available` on headless/SSH/WSL systems

**Solution**: Use pipe mode instead:
```bash
cat error.log | itk > cleaned.txt
```

### Wrong content type detected
**Problem**: ITK misdetects your content (e.g., treats code as plain text)

**Solution**: Force the type:
```bash
itk --type rust
itk --type json
itk --type yaml
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

---

## License

MIT — see [LICENSE](LICENSE)

---

## Contributing

Contributions welcome! Open an issue or PR on [GitHub](https://github.com/natekali/itk).

Issues: [github.com/natekali/itk/issues](https://github.com/natekali/itk/issues)
