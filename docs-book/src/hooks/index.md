# Hooks

Mneme integrates with AI hosts via 3 PreToolUse-style hooks plus the MCP server. Every hook is fail-open: if Mneme is down or misconfigured, the user's Edit/Write/Read still goes through. The hooks add value when they work; they never subtract value when they don't.

[Concept page →](../concepts/self-ping.md)

## The three layers

| Layer | Fires on | What it does |
|---|---|---|
| [Layer 1](./userprompt.md) | Every UserPromptSubmit | Classifies the prompt, injects a context reminder appropriate to the intent (resume / code / simple). |
| [Layer 2](./pretool-edit.md) | PreToolUse Edit / Write / MultiEdit | Blocks the edit if `blast_radius` wasn't run for the target file in the last 10 min; auto-runs it inline. |
| [Layer 3](./pretool-grep.md) | PreToolUse Grep / Read / Glob | Soft-redirect: never blocks, but injects a `find_references` / `blast_radius` hint when the input looks symbol-shaped. |

## Configuration

`~/.mneme/config.toml`:

```toml
[hooks]
inject_user_prompt_reminder = true       # Layer 1
enforce_blast_radius_before_edit = true  # Layer 2
enforce_recall_before_grep = true        # Layer 3 (default: true since v0.4.0)
blast_radius_freshness_seconds = 600     # Layer 2: 10 min freshness window
```

Set any to `false` to disable that layer. Defaults shipped with v0.4.0 are appropriate for normal use; the soft-redirect is non-blocking, so leaving Layer 3 ON is a free upgrade.

## Hook output protocol

Every hook writes a single line of JSON to stdout:

```json
{ "hook_specific": { "decision": "approve" } }
```

Layers 1 and 3 never block — they always emit `decision: "approve"`. Layer 2 may emit `decision: "block"` with a `reason`:

```json
{ "hook_specific": { "decision": "block", "reason": "..." } }
```

When a layer wants to inject context the AI sees alongside the result, it adds `additionalContext`:

```json
{ "hook_specific": { "decision": "approve", "additionalContext": "..." } }
```

## Fail-open guarantee

Every hook handler returns Ok(()) regardless of internal errors:

- Daemon down → approve
- Stdin parse fails → approve
- Config malformed → fall back to defaults, approve

For defense-in-depth, the CLI dispatch layer wraps each hook subcommand in `run_hook_failopen` (added in the v0.4.0 audit Wave 2 fix REL-001) — any future regression that propagates an error via `?` gets converted to a fail-open JSON envelope before reaching `main.rs`'s exit handler, preventing Claude Code from interpreting a non-zero exit as BLOCK.

## Windows GUI dispatcher

On Windows, the platform integration writes hook entries pointing at `mneme-hook.exe` (a separate GUI-subsystem binary) instead of `mneme.exe`. This eliminates the console-window flash on every PreToolUse fire. `mneme-hook.exe` delegates to the same `cli::commands::*::run` handlers `mneme.exe` uses, so the output semantics are identical.

[Layer 1 details →](./userprompt.md) · [Layer 2 details →](./pretool-edit.md) · [Layer 3 details →](./pretool-grep.md)
