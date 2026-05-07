# Self-ping enforcement (3 layers)

Mneme's job is to be the AI's persistent code-graph memory. But an AI that has Mneme installed will still default to grep + read if it forgets the tool exists. Genesis ships a 3-layer hook system that nudges the AI back toward Mneme on every turn — without ever blocking the user's workflow.

The motto: **soft-redirect, never block, fail-open.**

## Layer 1 — UserPromptSubmit

Fires on every user message. Classifies the prompt into one of three buckets:

| Intent | Trigger | Hook injects |
|----|----|----|
| **Resume** | `continue`, `resume`, `where was i`, `where were we`, `carry on`, `keep going`, `proceed` (+ length ≤ 40 chars to avoid misclassification) | Heavy block: `mneme_resume`, `step_status`, `step_show` |
| **Code** | Long prompt OR contains code-shaped keywords | Light block: top-3 tool reminder + grep/read trespass log |
| **Simple** | Short, no code keywords | Empty (no injection — the user is having a conversation) |

The classifier is conservative on the heavy bucket (only fires on the explicit resume signals listed) so AI sessions where the user said something like "yes continue" don't drown in injected context. The classifier is also conservative on the empty bucket — the default for ambiguous prompts is the light block.

[Source](https://github.com/omanishay-cyber/mneme/blob/main/cli/src/commands/userprompt_submit.rs) · [Item #119 in CHANGELOG](../releases/changelog.md)

## Layer 2 — PreToolUse Edit / Write / MultiEdit

Fires before every Edit, Write, and MultiEdit tool call. The hook checks whether the AI has run `mcp__mneme__blast_radius` for the target file in the last 10 minutes.

If yes: pass through. The AI already has the impact context.

If no: BLOCK the edit AND auto-run `blast_radius` inline so the AI's next turn has the answer. Within ~100 ms the AI sees the impact + can immediately retry the edit, this time informed.

For small files (< 100 LOC, set via the `SMALL_FILE_BYTE_THRESHOLD` constant) the gate is skipped — the cost of a blast-radius query exceeds the value when the blast radius is bounded by the file itself.

The configuration knob is `[hooks] enforce_blast_radius_before_edit = true|false` in `~/.mneme/config.toml`. Default true.

[Source](https://github.com/omanishay-cyber/mneme/blob/main/cli/src/commands/pretool_edit_write.rs) · [Item #120 in CHANGELOG](../releases/changelog.md)

## Layer 3 — PreToolUse Grep / Read / Glob

Fires before every Grep, Read, and Glob. The hook NEVER blocks — it always approves so the underlying tool still runs. But when the input looks like a code-symbol query (alphanumeric identifier, dotted/`::` module path, PascalCase type name) or a substantive source-file Read, the hook attaches a one-sentence `additionalContext` hint suggesting the equivalent Mneme tool:

```text
Grep("WorkerPool")
  →  hook approves Grep
     hook injects: "mneme tip: 'WorkerPool' looks like a code symbol —
                    mcp__mneme__find_references returns structured (file,
                    line, kind) hits with the symbol resolver applied,
                    typically faster + more precise than text grep. The
                    Grep is approved; prefer mneme on the next symbol
                    query."
```

```text
Read("supervisor/src/manager.rs")
  →  hook approves Read
     hook injects: "mneme tip: before non-trivial edits to
                    `supervisor/src/manager.rs`, mcp__mneme__blast_radius
                    returns the (callers, dependents, tests) set in <100 ms
                    — much cheaper than reading the file plus its consumers
                    manually. The Read is approved; consider blast_radius if
                    the next step is an edit."
```

The classifier is intentionally narrow:

- Multi-word phrases (multiple whitespace-separated tokens) → no hint (you're searching for natural-language text, not symbols)
- Regex metacharacters (`\[]()*+?|^${}`) → no hint (you're doing regex, not symbol lookup)
- Path-like inputs (contains `/`) → no hint (you're searching paths)
- Non-ASCII characters → no hint (security: avoids Unicode homoglyph + bidi-mark prompt injection)
- Length cap at 200 chars → no hint (anything longer isn't a symbol)

For Read: extension allow-list of `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.go`, `.java`, `.kt`, `.swift`, `.cpp`, `.cc`, `.c`, `.h`, `.hpp`, `.rb`, `.php`, `.cs`. README, JSON, HTML, PNG, etc. → no hint (you're reading data the user actually asked for).

The configuration knob is `[hooks] enforce_recall_before_grep = true|false`. Default true.

[Source](https://github.com/omanishay-cyber/mneme/blob/main/cli/src/commands/pretool_grep_read.rs) · [Item #122 in CHANGELOG](../releases/changelog.md)

## Fail-open guarantee

Every hook returns approve/empty if anything goes wrong:

- Daemon down → approve
- Hook subcommand panics → wrapped in `run_hook_failopen` → emits `{"hook_specific":{"decision":"approve"}, "_mneme_diag":"..."}` and exits 0
- Stdin parse fails → approve
- Config file malformed → fall back to defaults, approve
- Mneme MCP times out → approve

The user's session is NEVER bricked by Mneme. If you see any tool call blocked, it's intentional (Layer 2's blast-radius gate, when enabled) and Mneme tells the AI what to do to unblock.

## Why a Windows GUI binary

Claude Code spawns a hook process for every PreToolUse. On Windows, every command-line process gets a console window unless the binary is built for the GUI subsystem.

Genesis ships `mneme-hook.exe` — a separate Windows GUI-subsystem binary that handles the 3 hook subcommands without flashing a console. The platform integration writes hook entries pointing at `mneme-hook.exe`; everything else (`mneme build`, `mneme recall`, etc.) still uses the regular `mneme.exe` so terminal output works normally.

On Linux and macOS this binary is unnecessary — those OSes don't allocate console windows for child processes — but the build matrix produces it on all platforms for path-uniformity. The platform-integration code only swaps mneme → mneme-hook on Windows.

[Item #125 in CHANGELOG](../releases/changelog.md)

## Configuration

`~/.mneme/config.toml`:

```toml
[hooks]
# Layer 1
inject_user_prompt_reminder = true

# Layer 2
enforce_blast_radius_before_edit = true
blast_radius_freshness_seconds = 600   # 10 min

# Layer 3
enforce_recall_before_grep = true
```

Defaults shipped with Genesis. Set any to `false` to disable that layer.

[Layer 1 source](../hooks/userprompt.md) · [Layer 2 source](../hooks/pretool-edit.md) · [Layer 3 source](../hooks/pretool-grep.md)
