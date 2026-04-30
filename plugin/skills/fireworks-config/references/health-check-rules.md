# Health Check Validation Rules

## Severity Levels
- **ERROR**: Blocks functionality, must fix immediately
- **WARNING**: Suboptimal, should fix soon
- **INFO**: Optimization opportunity

## Rules

| ID | Check | Severity | Fix |
|---|---|---|---|
| HC-001 | CLAUDE.md under 200 lines | WARNING | Extract to rules/ or skills |
| HC-002 | MEMORY.md under 200 lines | WARNING | Trim index, use topic files |
| HC-003 | All hook scripts exist at paths in settings.json | ERROR | Fix paths or remove hook |
| HC-004 | All hook scripts have no syntax errors | ERROR | Fix Python/Bash syntax |
| HC-005 | settings.json parses as valid JSON | ERROR | Fix JSON syntax |
| HC-006 | No permission contradictions (allow vs deny) | ERROR | Remove conflicting entry |
| HC-007 | All agents have valid frontmatter (name, description) | WARNING | Add missing fields |
| HC-008 | All skills have SKILL.md with description | WARNING | Add description field |
| HC-009 | No path-unscoped rules over 50 lines | WARNING | Add globs: or split |
| HC-010 | Skill descriptions total under 16K chars | WARNING | Shorten descriptions |
| HC-011 | No duplicate content across CLAUDE.md and rules/ | INFO | Remove duplicate |
| HC-012 | No hook timeout over 600s | WARNING | Reduce timeout |
| HC-013 | SessionStart hooks complete under 5s | WARNING | Optimize script |
| HC-014 | No broken @imports in CLAUDE.md | ERROR | Fix path or remove |
| HC-015 | No Windows reserved filenames in config | ERROR | Rename file |

## Running a Health Check

```bash
# Quick validation commands:

# HC-005: settings.json valid
python -c "import json; json.load(open('$HOME/.claude/settings.json'))"

# HC-001: CLAUDE.md line count
wc -l ~/.claude/CLAUDE.md

# HC-002: MEMORY.md line count
wc -l ~/.claude/projects/*/memory/MEMORY.md

# HC-003: Hook scripts exist
grep -oP '"command":\s*"[^"]*"' ~/.claude/settings.json | \
  sed 's/"command":\s*"//;s/"$//' | \
  while read cmd; do
    script=$(echo "$cmd" | grep -oP '(?:python |bash )\K\S+')
    [ -n "$script" ] && [ ! -f "$script" ] && echo "MISSING: $script"
  done

# HC-007: Agent frontmatter
for f in ~/.claude/agents/*.md; do
  head -5 "$f" | grep -q "^name:" || echo "NO NAME: $f"
  head -10 "$f" | grep -q "^description:" || echo "NO DESC: $f"
done

# HC-008: Skill descriptions
for d in ~/.claude/skills/*/; do
  [ -f "$d/SKILL.md" ] || echo "NO SKILL.md: $d"
  head -10 "$d/SKILL.md" 2>/dev/null | grep -q "^description:" || echo "NO DESC: $d"
done

# HC-010: Total description characters
total=0
for d in ~/.claude/skills/*/SKILL.md; do
  desc=$(sed -n '/^description:/,/^[a-z]/p' "$d" | head -20 | wc -c)
  total=$((total + desc))
done
echo "Total skill description chars: $total / 16000"
```
