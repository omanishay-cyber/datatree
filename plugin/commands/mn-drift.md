---
name: /mn-drift
description: Show open drift findings — rule violations the scanners caught but no one has fixed yet.
command: mneme drift
---

# /mn-drift

Surface every open drift finding across the working tree. Drift findings
are produced by the scanners (theme, types, security, a11y, perf,
md-drift, ipc, secrets, refactor, architecture) and remain in the
findings shard until the underlying file is fixed and re-audited.

## Usage

```
/mn-drift                              # all open findings
/mn-drift --severity critical          # only red
/mn-drift --severity high              # red + yellow
/mn-drift --file <path>                # findings for one file
/mn-drift --scanner theme              # findings from one scanner
/mn-drift --json                       # machine-readable output
```

## What this does

1. Calls the `drift_findings(severity?, file?, scanner?)` MCP tool.
2. Renders findings as a table grouped by severity then by file.
3. Each row shows the rule name, message, and a short suggestion.

## Suggested workflow

- Daily standup: `/mn-drift --severity critical` before starting work.
- Before commit: `/mn-audit --scope diff` (re-runs scanners) then `/mn-drift`.
- After fixing: re-audit the file to clear the finding.

See also: `/mn-audit` (run scanners), the `mneme-audit` skill.
