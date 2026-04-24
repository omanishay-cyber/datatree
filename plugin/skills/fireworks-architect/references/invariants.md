# INVARIANTS.md — Complete Specification

## Purpose

INVARIANTS.md is a machine-verifiable contract file that lives at the root of a project. It defines architectural rules as executable checks. After every code change (Write or Edit operation), a hook reads INVARIANTS.md and runs each verification command. If any invariant fails, the change is flagged and must be fixed before proceeding.

This pattern prevents the most dangerous class of bugs: "fixed X but broke Y." By encoding architectural decisions as automated checks, violations are caught immediately — not days later during manual testing.

---

## File Format

INVARIANTS.md uses a specific, parseable format:

```markdown
# INVARIANTS — [Project Name]

## [Category Name]
- [ ] [Human-readable contract description]: `[shell command]` = [expected output]
- [ ] [Human-readable contract description]: `[shell command]` = [expected output]

## [Another Category]
- [ ] [Contract]: `[command]` = [expected]
```

### Format Rules

1. Each invariant is a markdown checkbox item (`- [ ]`)
2. The description comes first, followed by a colon
3. The verification command is wrapped in backticks
4. The expected output follows `=` (typically `0` for "no violations found")
5. Categories group related invariants for readability
6. Comments can be added as regular markdown text between categories

---

## Standard Invariant Categories

### Type Safety
```markdown
## Type Safety
- [ ] No `any` types in source: `grep -rn ": any" src/ --include="*.ts" --include="*.tsx" | grep -v "node_modules" | grep -v "// eslint-disable" | wc -l` = 0
- [ ] TypeScript compiles cleanly: `npx tsc --noEmit 2>&1 | grep "error TS" | wc -l` = 0
- [ ] No type assertions without comments: `grep -rn "as unknown as" src/ --include="*.ts" --include="*.tsx" | grep -v "// SAFETY:" | wc -l` = 0
```

### Security
```markdown
## Security
- [ ] No nodeIntegration enabled: `grep -rn "nodeIntegration: true" src/ | wc -l` = 0
- [ ] contextIsolation not disabled: `grep -rn "contextIsolation: false" src/ | wc -l` = 0
- [ ] No hardcoded secrets: `grep -rn "password\s*=\s*['\"]" src/ --include="*.ts" | wc -l` = 0
- [ ] No SQL string concatenation: `grep -rn "SELECT.*+.*'" src/ --include="*.ts" | wc -l` = 0
```

### IPC (Electron-Specific)
```markdown
## IPC
- [ ] All IPC handlers validate input: every `ipcMain.handle` call includes `.parse()` or `validate` within 5 lines
- [ ] No synchronous IPC: `grep -rn "sendSync\|invokeSync" src/ | wc -l` = 0
- [ ] All channels use domain:action naming: `grep -rn "ipcMain.handle\|ipcRenderer.invoke" src/ | grep -v "[a-z]*:[a-z]" | wc -l` = 0
- [ ] Preload exposes minimal API: count of `exposeInMainWorld` calls should not exceed documented limit
```

### Architecture
```markdown
## Architecture
- [ ] No circular dependencies: `npx madge --circular src/ 2>&1 | grep "Found" | wc -l` = 0
- [ ] No direct store access from main process: `grep -rn "useStore\|create(" src/main/ --include="*.ts" | wc -l` = 0
- [ ] No renderer accessing Node APIs: `grep -rn "require('fs')\|require('path')\|require('child_process')" src/renderer/ | wc -l` = 0
- [ ] All components use named exports: `grep -rn "export default" src/renderer/ --include="*.tsx" | wc -l` = 0
```

### Code Quality
```markdown
## Code Quality
- [ ] No console.log in production code: `grep -rn "console.log" src/ --include="*.ts" --include="*.tsx" | grep -v "// DEBUG" | grep -v "test" | wc -l` = 0
- [ ] No TODO without issue reference: `grep -rn "TODO" src/ --include="*.ts" --include="*.tsx" | grep -v "TODO(#[0-9])" | wc -l` = 0
```

### Database
```markdown
## Database
- [ ] All queries use parameterized inputs: `grep -rn "db.run\|db.exec\|db.prepare" src/ | grep -v "?" | grep "'" | wc -l` = 0
- [ ] Migrations are sequential: migration files follow `NNN_description.sql` naming
- [ ] Schema version tracked: `grep -rn "schema_version\|migration_version" src/ | wc -l` > 0
```

---

## Hook Integration

### check-invariants.sh

The hook that enforces INVARIANTS.md runs after every Write/Edit operation. It parses each invariant line, extracts the verification command from backticks, extracts the expected value after `=`, executes the command, and compares the result. If any invariant produces output that does not match the expected value, the hook reports the violation and exits with a non-zero status.

**How the hook works:**

1. Reads the INVARIANTS.md file line by line
2. Filters for lines matching the `- [ ]` checkbox pattern
3. Extracts the shell command between backticks using pattern matching
4. Extracts the expected value after the `=` sign
5. Runs the command via `bash -c` and captures its output
6. Compares actual output to expected output
7. Reports any mismatches as violations
8. Exits with failure status if any violations are found

### Hook Configuration

In `.claude/settings.json`, the hook is configured as a PostToolUse hook:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "command": "bash .claude/hooks/check-invariants.sh"
      }
    ]
  }
}
```

---

## Invariant Severity Levels

| Severity | Meaning | Action |
|----------|---------|--------|
| CRITICAL | Security vulnerability or data loss risk | Block commit, block deploy. Fix immediately. |
| HIGH | Architectural degradation or reliability risk | Block commit. Fix before merge. |
| MEDIUM | Code quality or maintainability concern | Warn in CI. Fix within the sprint. |

---

## Creating INVARIANTS for a New Project

### Step 1: Start Small
Begin with 5-10 core rules covering the most critical areas:
- Type safety (no `any`, tsc compiles)
- Security (no nodeIntegration, contextIsolation)
- Architecture (no circular deps, proper process isolation)

### Step 2: Add Rules Reactively
When a bug reveals a pattern that should have been caught:
1. Write a verification command that would have detected the bug
2. Add it to INVARIANTS.md
3. Verify it catches the current state correctly

### Step 3: Remove Stale Rules
Rules become stale when:
- The technology they check is no longer used
- The pattern they enforce has been superseded
- They produce too many false positives

### Step 4: Keep Commands Fast
Each verification command should complete in under 5 seconds. Slow checks create friction and discourage usage. If a check is slow:
- Use `grep` instead of full linter runs where possible
- Limit search scope to relevant directories
- Cache results where appropriate

---

## Best Practices

1. **Invariants are non-negotiable** — they represent architectural decisions, not suggestions
2. **Every invariant must have a verification command** — unverifiable rules are aspirations, not invariants
3. **Expected values must be deterministic** — avoid commands with non-deterministic output
4. **Document exceptions** — if a rule has known exceptions, note them in comments
5. **Review invariants quarterly** — remove stale rules, add new ones based on recent bugs
6. **Share across projects** — security and type safety invariants are often reusable
7. **Run invariants in CI** — not just locally, ensure CI also checks invariants on every push

---

## What Invariants Are NOT

- **Not tests** — tests verify behavior, invariants verify structure
- **Not lint rules** — lint rules check syntax, invariants check architecture
- **Not documentation** — if the verification command cannot prove it, it is not an invariant
- **Not aspirational** — every invariant must pass RIGHT NOW on the current codebase

---

## Troubleshooting

### "Invariant passes locally but fails in CI"
- Check for path differences (Windows vs Unix)
- Check for tool version differences (grep flags, node version)
- Ensure the command works in both Git Bash and standard bash

### "Too many false positives"
- Refine the grep pattern to be more specific
- Add exclusion patterns for known exceptions
- Consider using a more precise tool (AST parser vs regex)

### "Command is too slow"
- Limit scope: `src/` instead of `.`
- Use `--include` flags to target specific file types
- Replace full linter runs with targeted grep commands

---

## Example: Full INVARIANTS.md for your Electron project

```markdown
# INVARIANTS — your Electron project

## Type Safety
- [ ] No any types: `grep -rn ": any" src/ --include="*.ts" --include="*.tsx" | grep -v node_modules | wc -l` = 0
- [ ] tsc compiles: `npx tsc --noEmit 2>&1 | grep "error TS" | wc -l` = 0

## Security
- [ ] No nodeIntegration: `grep -rn "nodeIntegration: true" src/ | wc -l` = 0
- [ ] contextIsolation enabled: `grep -rn "contextIsolation: false" src/ | wc -l` = 0
- [ ] No sendSync: `grep -rn "sendSync" src/ | wc -l` = 0

## IPC
- [ ] All handlers validate: every ipcMain.handle has validation
- [ ] Channel naming: all channels follow domain:action format

## Architecture
- [ ] No circular deps: `npx madge --circular src/` reports no cycles
- [ ] Process isolation: no Zustand in main, no Node APIs in renderer

## Database
- [ ] Parameterized queries: no string concatenation in SQL
- [ ] Migrations versioned: schema_version table exists
```
