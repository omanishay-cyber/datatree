---
name: /dt-step
description: View, verify, complete, or resume the mneme Step Ledger. The compaction-resilient task tracker.
command: mneme step
---

# /dt-step

Interact with the mneme Step Ledger — the compaction-resilient task
tracker that survives context resets.

## Usage

```
/dt-step                       # show current step + brief ledger
/dt-step status                # explicit step_status call
/dt-step show <step_id>        # detail of one step
/dt-step verify <step_id>      # run the acceptance check
/dt-step complete <step_id>    # mark complete (only if verify passes)
/dt-step resume                # emit resumption bundle (after compaction)
/dt-step plan <markdown_path>  # ingest a roadmap into the ledger
/dt-step block <step_id> <reason>  # mark a step blocked
/dt-step unblock <step_id>     # resume a blocked step
```

## What this does

Routes to the matching MCP step tool:

| Sub-command | Tool |
|---|---|
| (default) / `status` | `step_status()` |
| `show` | `step_show(step_id)` |
| `verify` | `step_verify(step_id, dry_run?)` |
| `complete` | `step_complete(step_id, force?)` |
| `resume` | `step_resume(session_id?)` |
| `plan` | `step_plan_from(markdown_path, session_id?)` |

## Compaction resilience

Use `/dt-step resume` after **any** context reset. The resumption bundle
contains the original goal, completed steps with proofs, the current
step (YOU ARE HERE), planned steps, active constraints, and the
verification gate.

See also: the `mneme-resume` skill, and the `mneme-step-verifier`
sub-agent that runs your acceptance commands.
