# Layer 3 — PreToolUse Grep / Read / Glob

The soft-redirect layer. Never blocks — always approves. Injects a one-sentence `additionalContext` hint when the input looks symbol-shaped or points at a source file.

## Logic

```text
parse stdin payload
  ├── tool_name == "Grep"   → grep_hint(input)
  ├── tool_name == "Read"   → read_hint(input)
  ├── tool_name == "Glob"   → no hint (paths, not symbols)
  └── other                  → no hint
```

### Grep hint trigger

The pattern must pass `is_symbol_shaped`:

- ≤ 200 chars
- Single token (no whitespace)
- No regex metacharacters (`\\[]()*+?|^${}`)
- No path separators (`/`)
- ASCII-only (security: avoids Unicode homoglyph + bidi-mark prompt injection per Wave 2 audit fix SEC-006)
- Every char is `is_ascii_alphanumeric` OR `_` OR `:` OR `.`

So `WorkerPool`, `crate::manager::spawn`, `pkg.sub.mod`, `snake_case_name` all hint. `fn spawn`, `fn\\s+\\w+\\(`, `src/manager.rs`, `how does spawn work` all DON'T hint.

### Read hint trigger

The path must end with one of:

`.rs` `.ts` `.tsx` `.js` `.jsx` `.py` `.go` `.java` `.kt` `.swift` `.cpp` `.cc` `.c` `.h` `.hpp` `.rb` `.php` `.cs`

Markdown / JSON / HTML / images / configs → no hint (the user is reading data, not source).

## Output shape

```json
{
  "hook_specific": {
    "decision": "approve",
    "additionalContext": "mneme tip: \"WorkerPool\" looks like a code symbol — \
                          mcp__mneme__find_references returns structured (file, \
                          line, kind) hits with the symbol resolver applied, \
                          typically faster + more precise than text grep. The \
                          Grep is approved; prefer mneme on the next symbol query."
  }
}
```

When no hint applies:

```json
{ "hook_specific": { "decision": "approve" } }
```

The hook never adds a `_truncated` envelope — hint strings are bounded at 80/120 chars by `sanitize_for_message`.

## Configuration

```toml
[hooks]
enforce_recall_before_grep = true     # default true since v0.4.0
```

The default flipped to TRUE in v0.4.0 to align with the Rust + TS bindings (the v0.4.0 audit Wave 2 fix REL-002 caught a prior false-on-TS / true-on-Rust mismatch). Set to false for legacy passthrough.

## Source

[`cli/src/commands/pretool_grep_read.rs`][src] — 380 LOC + 15 tests covering symbol-shape acceptance/rejection, source-file recognition, JSON envelope shape, and the no-hint paths.

[src]: https://github.com/omanishay-cyber/mneme/blob/main/cli/src/commands/pretool_grep_read.rs

## Security notes

The hint string interpolates the user-controlled pattern + path. To prevent prompt injection via crafted inputs:

- Pattern goes through `sanitize_for_message` which strips control chars + truncates to 80 chars
- Path goes through the same sanitizer with a 120-char cap
- ASCII-only allow-list at `is_symbol_shaped` blocks Unicode lookalikes (Cyrillic 'а' for Latin 'a', RTL-override marks, zero-width joiners)
- Stdin reads cap at 1 MiB so a sibling process can't OOM the hot path
- config.toml reads cap at 64 KiB pre-flight before TOML parsing

These defenses landed in the Wave 2 audit fixes (SEC-001, SEC-006, SEC-007).
