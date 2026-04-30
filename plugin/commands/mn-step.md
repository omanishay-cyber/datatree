---
name: /mn-step
description: View, verify, complete, or resume the mneme Step Ledger. The compaction-resilient task tracker.
command: mneme step
---

# /mn-step

Interact with the mneme Step Ledger — the compaction-resilient task
tracker that survives context resets.

## Usage

```
/mn-step                       # show current step + brief ledger
/mn-step status                # explicit step_status call
/mn-step show <step_id>        # detail of one step
/mn-step verify <step_id>      # run the acceptance check
/mn-step complete <step_id>    # mark complete (only if verify passes)
/mn-step resume                # emit resumption bundle (after compaction)
/mn-step plan <markdown_path>  # ingest a roadmap into the ledger
/mn-step block <step_id> <reason>  # mark a step blocked
/mn-step unblock <step_id>     # resume a blocked step
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

Use `/mn-step resume` after **any** context reset. The resumption bundle
contains the original goal, completed steps with proofs, the current
step (YOU ARE HERE), planned steps, active constraints, and the
verification gate.

See also: the `mneme-resume` skill, and the `mneme-step-verifier`
sub-agent that runs your acceptance commands.
