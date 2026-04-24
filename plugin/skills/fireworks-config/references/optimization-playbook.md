# Context Budget Optimization Playbook

## Context Budget Formula

```
Total Always-Loaded Context =
  CLAUDE.md tokens (~2,000)
  + Non-path-scoped rules/ tokens (~200 each)
  + Skill description metadata (~100 chars each × N skills)
  + MEMORY.md first 200 lines (~1,500 tokens)
  + @imported file tokens (varies)

Target: < 5% of context window for config overhead
```

## Optimization Strategies (Ranked by Impact)

### 1. Path-Scope Your Rules (HIGH impact)
Rules without `globs:` load every session. Add path-scoping to domain-specific rules:

```markdown
# Before (always loaded)
---
description: TypeScript strict patterns
---

# After (only loaded for .ts files)
---
description: TypeScript strict patterns
globs: ["**/*.ts", "**/*.tsx"]
---
```

Savings: ~200 tokens per rule × number of rules not relevant to current task.

### 2. Remove CLAUDE.md Content Enforced by Hooks (MEDIUM impact)
If a hook already enforces a rule deterministically, the CLAUDE.md line is redundant:

| CLAUDE.md Rule | Hook That Enforces It | Action |
|---|---|---|
| "Read before write" | protect-files.py checks file read history | Remove from CLAUDE.md |
| "Never kill node.exe" | block-dangerous.py blocks the command | Remove from CLAUDE.md |
| "Test after changes" | check-test-exists.sh reminds | Keep (hook is reminder, not enforcer) |

### 3. Shorten Skill Descriptions (LOW-MEDIUM impact)
Each skill description contributes to the 2% context budget. Trim verbose descriptions:

```yaml
# Before (180 chars)
description: >-
  Comprehensive system design and architecture planning with RPI methodology,
  INVARIANTS contracts, sprint decomposition, and Electron architecture patterns
  for premium desktop applications.

# After (120 chars)
description: >-
  System design — RPI methodology, INVARIANTS contracts, sprint decomposition,
  Electron architecture. Use for architecture planning and system design.
```

### 4. Add disable-model-invocation to Manual-Only Skills (LOW impact)
Skills that should only be invoked via `/skill-name` (not auto-selected by Claude):

```yaml
disable-model-invocation: true
```

This removes the skill from Claude's auto-selection, reducing description scanning.

### 5. Extract Auto-Dispatch to a Skill (LOW impact)
The ~25-line Auto-Dispatch Rules in CLAUDE.md loads every session. Move to a `user-invocable: false` skill that Claude loads when deciding dispatch. Saves ~500 tokens.

## Measuring Your Budget

```bash
# Count CLAUDE.md tokens (rough: 1 token ≈ 4 chars)
wc -c ~/.claude/CLAUDE.md | awk '{print int($1/4), "tokens"}'

# Count non-path-scoped rules
for f in ~/.claude/rules/*.md; do
  if ! head -5 "$f" | grep -q "globs:"; then
    chars=$(wc -c < "$f")
    echo "ALWAYS LOADED: $f (~$((chars/4)) tokens)"
  fi
done

# Count skill description total
total=0
for d in ~/.claude/skills/*/SKILL.md; do
  desc_chars=$(sed -n '/^description:/,/^[a-z]/p' "$d" 2>/dev/null | wc -c)
  total=$((total + desc_chars))
done
echo "Skill descriptions: $total chars (budget: 16000)"
```
