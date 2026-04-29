# Open Session Detection Strategies

This document describes how AI Shepherd currently detects open or recent AI coding sessions, what signal each strategy uses, and where the current approach can fail.

## Goal

AI Shepherd needs to answer two different questions:

1. **Which AI tool panes are currently visible in the terminal?**
2. **Which AI coding sessions have recent or current usage activity?**

Those are related, but not equivalent. A terminal pane can be open without usage events, and a usage session can exist without a currently visible terminal pane.

## Current detection layers

### 1. Terminal pane detection

Implemented in `src-tauri/src/terminal.rs` through the WezTerm adapter.

The app lists WezTerm panes, captures a small sample of pane output, and tries to classify the pane as one of the known AI tools.

Signals used:

- pane title
- current working directory, when reported
- foreground process name, when available
- captured terminal text sample

Known tool keywords:

- OpenCode: `opencode`, `open code`, `open-code`
- Claude: `claude`
- Codex: `codex`
- Gemini: `gemini`

Priority is important. OpenCode is checked before Claude, Codex, and Gemini. Codex is intentionally not matched from captured pane output because OpenCode sessions backed by GPT/Codex can print Codex-related text and otherwise produce false duplicates.

Strengths:

- Detects live panes even before usage events are written.
- Works without parsing provider-specific session databases.
- Helps the UI show shell, cwd, process, and terminal adapter context.

Weaknesses:

- Keyword matching is heuristic.
- Terminal output can contain names of tools that are not the active tool.
- WezTerm capture can fail or be incomplete.
- Process names are currently often unavailable, so title/cwd/sample carry most of the signal.

### 2. Claude usage/session detection

Implemented in `src-tauri/src/usage.rs` through `ClaudeCodeUsageAdapter`.

The adapter reads two Claude sources:

- `~/.claude/projects/**/*.jsonl`
- `~/.claude/history.jsonl`

Project JSONL files provide token-bearing usage events. The parser extracts:

- timestamp
- session id
- session title, from explicit title entries or the first user message
- model, when present
- input/output token counts

`history.jsonl` provides lightweight session entries. This is needed for sessions that exist but have no token usage yet, for example a freshly opened Claude session where the user has not typed anything meaningful.

History entries are represented as zero-usage events:

- `input_tokens = 0`
- `output_tokens = 0`
- `requests_used = 0`

Strengths:

- Captures real usage from Claude session logs.
- Can now surface empty/recent Claude sessions that do not have token events yet.
- Avoids depending only on terminal pane detection.

Weaknesses:

- Session logs are local implementation details and can change.
- `history.jsonl` has less detail than project JSONL files.
- Empty sessions may be ambiguous because they have no usage activity.

### 3. Codex/GPT usage/session detection

Implemented in `src-tauri/src/usage.rs` through `CodexUsageAdapter`.

The adapter reads:

- `~/.codex/sessions/**/*.jsonl`

Codex session files contain cumulative `token_count` snapshots. AI Shepherd calculates per-event usage by subtracting the previous cumulative snapshot from the current one.

For example:

- first snapshot total: 10k input, 1k output
- next snapshot total: 12k input, 1.5k output
- stored delta: 2k input, 0.5k output

This avoids double-counting cumulative totals.

The parser also extracts:

- session id from `session_meta` or file name
- first user message as a session label
- model when available
- timestamp from the JSONL event timestamp

Strengths:

- Supports Codex/GPT usage without requiring a separate UI path.
- Handles cumulative token reporting safely through deltas.
- Integrates into the same SQLite usage table and summary model as Claude.

Weaknesses:

- Depends on the current Codex JSONL structure.
- Token snapshots are not the same shape as Claude usage entries.
- If Codex changes from cumulative to per-event usage, the delta logic would need revisiting.

## Summary generation

Usage events from Claude and Codex are persisted into SQLite in the shared `usage_events` table.

Summaries are grouped by provider and session id. If a session id is missing, the source file is used as the fallback group key.

The current summary window prefers:

1. events in the last five hours, if any exist;
2. otherwise, events from the latest available date;
3. otherwise, all untimestamped events.

Zero-token events are included so that open sessions without usage can appear in Recent Providers.

## Current false-positive controls

### OpenCode backed by GPT/Codex

Problem: an OpenCode pane connected to GPT/Codex may contain the word `Codex` in terminal output.

Mitigation: Codex is not detected from captured terminal samples. It is only detected from stronger signals such as title, cwd, or process name.

### Subagent noise in Claude logs

Problem: Claude subagent JSONL files can look like active sessions but are not first-class user sessions.

Mitigation: Claude project files under `subagents` are ignored for recent session files.

## Design trade-offs

The system intentionally combines two imperfect views:

- **terminal panes** are good for live presence;
- **usage files** are good for actual provider/session history.

Neither source is authoritative on its own. The current approach is pragmatic: show what can be inferred locally, preserve diagnostic metadata, and avoid pretending that heuristic matches are certain.

## Recommended next improvements

1. Add a confidence score to pane detection, for example `strong`, `medium`, `weak`.
2. Split terminal pane identity from backing model/provider. Example: `tool = OpenCode`, `backend = GPT-5.5`.
3. Store provider-specific diagnostics separately instead of aggregating everything into a single `all` diagnostic record.
4. Add fixtures and Rust tests for Claude project logs, Claude history logs, and Codex cumulative token snapshots.
5. Prefer process-name detection once WezTerm or platform APIs expose it reliably.
