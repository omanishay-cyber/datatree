---
name: fireworks-config
description: >-
  Manages and optimizes Claude Code configuration — CLAUDE.md, settings.json, hooks, rules, agents, skills, memory, and plugins. Audits for bloat, contradictions, dead config, and context budget waste. Use when checking config health, optimizing context budget, migrating rules between layers, creating new hooks/rules/skills, or syncing config across machines.
version: 1.0.0
author: mneme
tags: [config, CLAUDE.md, settings, hooks, rules, memory, plugins, optimization]
triggers: [config, settings, CLAUDE.md, hooks, rules, memory, optimize, health check, context budget]
---

# Fireworks Config — Claude Code Configuration Manager

## Overview

Single source of truth for managing the entire Claude Code configuration stack. Audits health, optimizes context budget, migrates rules between layers, and maintains consistency across machines.

**Core principle**: CLAUDE.md is advisory, hooks are deterministic. Never put in CLAUDE.md what a hook can enforce.

## The 7-Layer Config Hierarchy

| Layer | Location | Load Timing | Enforcement |
|-------|----------|-------------|-------------|
| 1. Managed Policy | Enterprise admin | Every session | Deterministic |
| 2. Enterprise Skills | Org-provided | Every session | Advisory |
| 3. User Config | `~/.claude/` | Every session | Advisory + Hooks |
| 4. Project Config | `.claude/` | Every session | Advisory + Hooks |
| 5. Local Config | `.claude/settings.local.json` | Every session | Deterministic |
| 6. Plugin-provided | Plugin cache | On plugin load | Varies |
| 7. Session-scoped | In-memory | Current session | Ephemeral |

Precedence: Higher number overrides lower for same key. See `references/config-hierarchy.md` for full docs.

## What Belongs Where — Decision Matrix

| Content Type | Best Layer | Why |
|---|---|---|
| Universal behavior guidance | CLAUDE.md | Loaded every session, natural language |
| Domain-specific guidance | `.claude/rules/` with `paths:` | Only loads for matching files |
| Must-happen-every-time gates | Hooks (PreToolUse/Stop) | Deterministic, exit code 2 blocks |
| On-demand workflows/knowledge | Skills | Loaded only when invoked |
| Permissions, plugins, technical | `settings.json` | Zero context cost |
| Project-specific conventions | Project `.claude/CLAUDE.md` | Shared via git |
| Machine-specific overrides | `settings.local.json` | Not shared via git |

**Rule of thumb**: If you can enforce it with a hook, don't put it in CLAUDE.md. If it only matters for certain file types, use path-scoped rules.

## Quick Commands

### Health Check
Validate entire config stack:
1. All hook scripts exist and are valid Python/Bash
2. settings.json parses as valid JSON
3. All agents have valid frontmatter (name, description, tools)
4. All skills have SKILL.md with name + description
5. CLAUDE.md under 200 lines
6. MEMORY.md under 200 lines
7. No broken @imports

See `references/health-check-rules.md` for all 15 validation rules.

### Context Budget Audit
Calculate total always-loaded token cost:
```
Total = CLAUDE.md tokens
      + SUM(non-path-scoped rules/ tokens)
      + SUM(skill descriptions, capped at 2% context window)
      + MEMORY.md first 200 lines
      + @imported file tokens

Target: < 5% of context window for config overhead
```

See `references/optimization-playbook.md` for reduction strategies.

### Contradiction Check
Cross-reference for conflicts:
- CLAUDE.md rules vs hook enforcement (duplicates waste tokens)
- rules/ files vs CLAUDE.md (conflicting guidance)
- settings.json permissions vs hook restrictions

## Config Creation Quick Templates

### New Path-Scoped Rule
```markdown
---
description: Brief description of what this rule enforces
globs: ["src/components/**/*.tsx"]
---

# Rule Name

When working on [domain]:
1. First rule
2. Second rule
```

### New Hook (PreToolUse)
```json
{
  "matcher": "Edit|Write",
  "hooks": [{
    "type": "command",
    "command": "python C:/Users/USERNAME/.claude/hooks/my-hook.py"
  }]
}
```
Hook script: read JSON from stdin, exit 0 (allow) or exit 2 (block with stderr message).

### New Skill
```
~/.claude/skills/skill-name/
├── SKILL.md          # Hub (<500 lines)
└── references/       # Deep content
```
Frontmatter: `name` (kebab-case), `description` (third-person, triggers + actions, <1024 chars).

## Path-Scoped Rules Optimization

Your rules that could benefit from path-scoping:

| Rule File | Suggested `globs:` |
|---|---|
| `electron-react.md` | `["**/*.ts", "**/*.tsx"]` |
| `security.md` | `["**/*.ts", "**/*.tsx", "**/*.js"]` |
| `testing.md` | `["**/*.test.*", "**/*.spec.*"]` |
| `typescript.md` | `["**/*.ts", "**/*.tsx"]` |
| `ui-premium.md` | `["**/*.tsx", "**/*.css", "**/*.scss"]` |

Adding `globs:` means these only load when Claude works on matching files — saves ~500 tokens per session when not relevant.

## Multi-Machine Sync

Three machines: Home (the user), Office (), Current ().

**Quick sync approach**: Git dotfiles repo
```bash
# Export
cp -r ~/.claude/skills/ ~/dotfiles/claude/skills/
cp -r ~/.claude/agents/ ~/dotfiles/claude/agents/
cp ~/.claude/settings.json ~/dotfiles/claude/
cp ~/.claude/CLAUDE.md ~/dotfiles/claude/
git -C ~/dotfiles add -A && git -C ~/dotfiles commit -m "sync claude config"

# Import (on other machine)
git -C ~/dotfiles pull
bash ~/dotfiles/claude/install-home.sh
```

See `references/migration-patterns.md` for advanced sync (chezmoi, path remapping).

## Scope Boundaries

- **MINIMUM**: Always check settings.json validity + hook script existence
- **MAXIMUM**: Do not rewrite CLAUDE.md without user approval. Do not modify hooks without testing.

## Verification Gates

Before claiming config changes are complete:
1. `python -c "import json; json.load(open(os.path.expanduser('~/.claude/settings.json')))"` — valid JSON
2. All hook scripts referenced in settings.json exist at their paths
3. No Python syntax errors in hook scripts: `python -c "import py_compile; py_compile.compile('hook.py')"`
4. Skill descriptions total under 16K characters

## ENABLE_TOOL_SEARCH

Add to settings.json env: `"ENABLE_TOOL_SEARCH": "true"`
- MCP tools load on demand instead of every conversation
- Saves tokens when multiple MCP servers are configured (Context7, Chrome DevTools)
- Officially supported since Claude Code 2.1.7

---

## Hook Health Monitoring

For all 16+ hooks, enforce:
- **Timeout isolation**: If any hook hangs >5 seconds, kill it and continue
- **Health metrics**: Track per-hook call count, error rate, avg duration
- **Ring buffer audit**: Last 10,000 hook events logged for debugging
- **Failure modes**: Each hook declares `:halt` (block on failure) or `:log_and_continue`

---

## Orchestrator Context Protection

When using multi-agent workflows:
- Orchestrator MUST NOT read implementation files
- Orchestrator MUST NOT read evaluation reports
- Parse ONLY structured headers (VERDICT, SCORE, ISSUES)
- This prevents context window overflow in complex agent chains

---

## Dynamic Context Injection (DCI)

Skills can pre-load environment state at activation time:
- Node version, git branch, Electron version, TypeScript config
- Loaded BEFORE Claude processes the skill — zero tool calls burned
- Pattern: include shell commands in skill that output context

---

## Related Skills

- **fireworks-workflow** — project lifecycle that config supports
- **fireworks-security** — security hooks and hardening config
- **fireworks-devops** — CI/CD that consumes config
