# Config Hierarchy — Full Documentation

## Layer 1: Managed Policy
- Set by enterprise admins via Anthropic console
- Cannot be overridden by any other layer
- Controls: model access, allowed tools, content policies
- Token cost: zero (injected server-side)

## Layer 2: Enterprise Skills
- Organization-provided skills distributed to all users
- Loaded automatically, cannot be disabled by users
- Useful for: company coding standards, compliance rules

## Layer 3: User Config (`~/.claude/`)
Primary user-level configuration:

| File/Dir | Purpose | Token Cost |
|---|---|---|
| `CLAUDE.md` | Global instructions | ~2K tokens (always loaded) |
| `settings.json` | Permissions, hooks, plugins | Zero (not in context) |
| `agents/*.md` | Agent definitions | Zero until dispatched |
| `skills/*/SKILL.md` | Skill descriptions | ~100 chars each (L1 metadata) |
| `rules/*.md` | Global or path-scoped rules | Varies (see below) |
| `hooks/*.py` | Hook scripts | Zero (runs externally) |
| `projects/*/memory/` | Auto-memory per project | First 200 lines of MEMORY.md |

### Rules Loading Behavior
- Rules WITHOUT `globs:` frontmatter → loaded EVERY session (same as CLAUDE.md)
- Rules WITH `globs:` frontmatter → loaded ONLY when Claude works on matching files

```markdown
---
description: TypeScript strict patterns
globs: ["**/*.ts", "**/*.tsx"]
---
# Only loads when editing .ts/.tsx files
```

## Layer 4: Project Config (`.claude/`)
Project-level configuration, shared via git:

| File | Purpose |
|---|---|
| `CLAUDE.md` | Project-specific instructions |
| `settings.json` | Project permissions (merged with user) |
| `rules/*.md` | Project-scoped rules |
| `skills/*/SKILL.md` | Project-specific skills |

## Layer 5: Local Config
- `.claude/settings.local.json` — machine-specific overrides
- NOT committed to git
- Use for: local paths, machine-specific hooks, env-specific settings

## Layer 6: Plugin-Provided
- Installed plugins provide their own agents, skills, hooks
- Loaded based on `enabledPlugins` in settings.json
- Cache location: `~/.claude/plugins/`

## Layer 7: Session-Scoped
- In-memory only, lost on session end
- Includes: task lists, plan mode state, conversation context
- Use `/clear` to reset

## Skill Description Budget
- Skills budget: 2% of context window (~16K characters fallback)
- Override: set `SLASH_COMMAND_TOOL_CHAR_BUDGET` env var
- If exceeded: some skill descriptions may be truncated or excluded
- Check: run `/context` to see if skills are being excluded

## Loading Order
1. Managed policy CLAUDE.md (if any)
2. User `~/.claude/CLAUDE.md`
3. User `~/.claude/rules/*.md` (non-path-scoped ones)
4. Project `.claude/CLAUDE.md`
5. Project `.claude/rules/*.md` (non-path-scoped ones)
6. All skill L1 metadata (name + description)
7. MEMORY.md (first 200 lines)
8. Path-scoped rules (on demand)
9. Full skill content (on demand)
