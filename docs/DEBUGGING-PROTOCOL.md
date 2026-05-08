# 🛑 Mneme Debugging Protocol (Master)

> **CRITICAL DIRECTIVE.** Whenever instructed to debug, fix, or refactor code in this repo, follow this exact 4-phase sequence.
>
> - **EXPRESSLY FORBIDDEN** from writing the final code fix until Phase 1 and Phase 2 are complete.
> - **EXPRESSLY FORBIDDEN** from presenting the fix to the user until Phase 3's actual type-checker run is GREEN.
>
> **Trigger:** "Follow the Debugging Protocol" or `/debug-protocol <module>`.

This file is the contract that AI agents (Claude Code, Cursor, Windsurf, Copilot) and human contributors use when fixing bugs in this codebase. It pairs with [`docs/audit/AUDIT-2026-05-07-FINDINGS.md`](audit/AUDIT-2026-05-07-FINDINGS.md) (the bug tracker).

---

## PHASE 1 — BLAST RADIUS ANALYSIS (tools-driven, no code yet)

Before writing any code, evaluate the connective tissue using **actual tools**, not mental simulation.

### 1.1 Identify Upstream

List every external API, state store, utility function, or external crate/package the target module relies on. Source = the `use` / `import` block at the top of the file.

### 1.2 Identify Downstream

List **every file** that imports this module or consumes its exported types/functions. Required tool order:

1. `mcp__tree-sitter__find_usage` (semantic, preferred).
2. `mcp__mneme__find_references` (when the mneme MCP daemon is up).
3. `grep -rn "use crate::<module>" src/` / `grep -rn "from <module>" src/` (fallback).

Output the actual file paths + line numbers. Never "I think nothing imports it."

### 1.3 State the Risk

Explicitly describe how changing the target file's internal logic might cause a cascade failure in the downstream consumers identified in 1.2.

> Example: *"If I change `Foo::method`'s return type from `Result<T>` to `Option<T>`, then `caller.rs:88` and `other.rs:14` would fail to compile because they `?`-propagate the Err."*

---

## PHASE 2 — CONTRACT LOCKDOWN

Treat module boundaries as immutable.

- **Zero Signature Changes.** May NOT alter function signatures, exported interfaces, type definitions, or return types unless explicitly authorized by the user.
- **Preserve Data Flow.** If the module currently returns a specific data structure (mapped object, strictly-typed array, JSON shape), the fix MUST adhere exactly to that shape.
- **Halt on Conflict.** If the bug *cannot* be fixed without breaking the interface contract, **STOP** and output:
  > "ARCHITECTURAL CONFLICT: this fix requires changing `<symbol>`'s signature from `<old>` to `<new>`. Downstream callers `<list>` would need updates. Do you authorize the interface change?"

  Wait for explicit authorization before proceeding.

---

## PHASE 3 — ACTUAL DRY-RUN (compiler-verified, not mental)

Do **not** rely on mental simulation alone.

### 3.1 Mental dry-run first (cheap)

Walk the planned change against each downstream consumer from 1.2. Catch obvious breaks before paying compile cost.

### 3.2 Write the fix

Apply the minimal change to the target file.

### 3.3 Run the type-checker IMMEDIATELY (full workspace)

| Stack | Command |
|---|---|
| Rust | `cargo check --workspace` |
| TypeScript | `bunx tsc --noEmit` (from `vision/` and `mcp/`) |
| Python | `mypy <module>` or `python -m py_compile <module>` |

### 3.4 If the compiler throws cross-module interface errors

- **UNDO THE CHANGE** (`git checkout -- <file>` or `git stash`).
- Re-evaluate the Phase 1.2 dependency map — what did you miss?
- Form a new hypothesis and try again.
- **DO NOT present the fix to the user until the compiler passes on the WHOLE WORKSPACE.**

### 3.5 Run module + integration tests

After type-check passes:

- `cargo test -p <crate> --lib <module>` / `vitest run <module>` / `pytest tests/<module>.py`.
- Then integration tests for downstream consumers from 1.2.
- All must pass before Phase 4.

### 3.6 Mneme repo's six pre-push gates (mandatory before any push)

1. `cargo fmt --all -- --check`
2. `cargo check --workspace`
3. `cargo test --workspace --no-run`
4. `bash scripts/check-home-dir-discipline.sh`
5. `cd vision && bunx tsc --noEmit`
6. `cd mcp && bun install --frozen-lockfile && bunx tsc --noEmit`

---

## PHASE 4 — REQUIRED OUTPUT FORMAT

When outputting the corrected code, response **MUST** follow this exact structure:

### 1. The Fix
[Provide the corrected code block — show diff or full new file region]

### 2. Dependency Safety Proof
[1-2 sentences explaining EXACTLY why this change will NOT break the downstream files identified in Phase 1.2. Cite file paths.]

### 3. Verification Command
[The specific terminal command, type-check, or test run the developer should execute to verify the fix and prove no dependencies were broken. Must include the EXACT command, not "run cargo test".]

### Example Phase 4 output

```
### 1. The Fix
   [diff of `cli/src/commands/foo.rs:42-67`]

### 2. Dependency Safety Proof
   `Foo::bar` returns `Result<Item, Error>` — unchanged. The 3 downstream
   callers (`caller.rs:88`, `other.rs:12`, `tests.rs:202`) `?`-propagate
   that Result and are unaffected by the internal logic refactor.

### 3. Verification Command
   cd source && cargo check --workspace && cargo test -p mneme-cli --lib foo
```

---

## Trigger phrases & enforcement

| Trigger | Behavior |
|---|---|
| "Follow the Debugging Protocol" | Run all 4 phases without negotiation. |
| `/debug-protocol <module>` | Same as above, with `<module>` as Phase 1 target. |
| "I'm getting an error in `<file>`. Follow the Debugging Protocol." | Same. |

## Halt conditions (STOP and ask the user)

- **Phase 2 conflict** — signature change required. Don't fudge it.
- **Phase 3 type-checker fails 3+ times** despite Phase 1.2 re-evaluation. This is an architectural problem; escalate, don't keep patching.

## When dispatching a sub-agent

Every Agent prompt **must** include this directive verbatim near the top:

> **You MUST follow the Mneme Debugging Protocol (`docs/DEBUGGING-PROTOCOL.md`). Phase 1 (deps map via tree-sitter find_usage) → Phase 2 (signatures frozen unless escalated) → Phase 3 (run cargo check + tests, undo on cross-module errors) → Phase 4 (structured output: Fix / Safety Proof / Verification Command). FORBIDDEN to present the fix until Phase 3 cargo check is green on the whole workspace.**

---

*Sources: original Gemini protocol + Claude Code CLI edition + Mneme's existing pre-push gate discipline. Mixed master version 2026-05-07.*
