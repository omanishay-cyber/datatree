# Config Migration Patterns

## CLAUDE.md → Path-Scoped Rule

When to migrate: Content that only applies to specific file types.

### Steps
1. Identify the CLAUDE.md section to extract
2. Create `.claude/rules/rule-name.md` with `globs:` frontmatter
3. Copy content verbatim
4. Remove from CLAUDE.md
5. Verify: edit a matching file — rule should appear in context

### Example
```markdown
# From CLAUDE.md:
## Coding Standards
- TypeScript strict mode
- No any types
- Named exports preferred

# To .claude/rules/typescript.md:
---
description: TypeScript coding standards
globs: ["**/*.ts", "**/*.tsx"]
---

- TypeScript strict mode — tsc --noEmit check before completing
- No any types — use unknown + type guards
- Named exports preferred over default exports
```

## CLAUDE.md → Hook

When to migrate: Rules that MUST be enforced deterministically (not just advised).

### Steps
1. Identify the rule to enforce
2. Write a hook script (Python recommended for complex logic)
3. Add hook to settings.json in the appropriate event
4. Remove advisory version from CLAUDE.md (or keep as documentation)
5. Test: trigger the condition — hook should block or warn

### Hook Script Template
```python
#!/usr/bin/env python3
import json, sys

input_data = json.load(sys.stdin)
tool_input = input_data.get("tool_input", {})

# Your check logic here
should_block = False
reason = ""

if should_block:
    print(reason, file=sys.stderr)
    sys.exit(2)  # Block the action

sys.exit(0)  # Allow the action
```

## Single-File Skill → Hub + References

When to migrate: Skill SKILL.md exceeds 500 lines.

### Steps
1. Identify sections that are deep-reference material (code examples, lookup tables, detailed workflows)
2. Create `references/` directory
3. Move each section to its own reference file
4. Replace in SKILL.md with a brief summary + cross-reference link
5. Keep SKILL.md under 500 lines as the "hub"

### Hub Pattern
```markdown
## Error Patterns

14 common patterns with fixes. Quick-reference:

| Pattern | Likely Cause | Fix |
|---|---|---|
| Cannot read undefined | Null access | Optional chaining |
| ... | ... | ... |

For detailed patterns with code examples, see `references/error-lookup-table.md`.
```

## Multi-Machine Sync

### Simple: Git Dotfiles
```bash
# One-time setup
mkdir -p ~/dotfiles/claude
cd ~/dotfiles && git init

# Export (run on source machine)
cp -r ~/.claude/skills/ ~/dotfiles/claude/skills/
cp -r ~/.claude/agents/ ~/dotfiles/claude/agents/
cp -r ~/.claude/hooks/ ~/dotfiles/claude/hooks/
cp -r ~/.claude/rules/ ~/dotfiles/claude/rules/
cp ~/.claude/settings.json ~/dotfiles/claude/settings.json.template
cp ~/.claude/CLAUDE.md ~/dotfiles/claude/CLAUDE.md
# Note: settings.json needs path patching per machine

# Import (run on target machine)
# Use install-home.sh which patches paths for local username
bash ~/dotfiles/claude/install-home.sh
```

### Advanced: chezmoi + Age
For encrypted secrets and automatic path remapping:
```bash
chezmoi init
chezmoi add ~/.claude/CLAUDE.md
chezmoi add --encrypt ~/.claude/settings.json  # Encrypt sensitive config
chezmoi cd  # Edit templates for path differences
chezmoi apply  # Apply on any machine
```

### Path Remapping
Settings.json contains hardcoded paths like `C:/Users//`. When syncing:
- Replace username in hook paths: `sed -i "s//$USER/g" settings.json`
- Or use `$HOME` in hook commands where supported
- The `install-home.sh` script handles this automatically
