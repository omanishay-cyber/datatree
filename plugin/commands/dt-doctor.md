---
name: /dt-doctor
description: Run mneme's full self-test suite — IPC round-trip, shard integrity, schema versions, worker health, SLA snapshot.
command: mneme doctor
---

# /dt-doctor

Run mneme's self-test suite and emit a structured health report.

## Usage

```
/dt-doctor                # full report
/dt-doctor --json         # machine-readable
/dt-doctor --quick        # IPC + worker status only (skip integrity)
/dt-doctor --shard graph  # one shard
```

## What this does

Calls the `doctor()` MCP tool, which runs:

1. **IPC round-trip** — supervisor reachable, latency under budget?
2. **Shard integrity** — `PRAGMA integrity_check` on every shard.
3. **Schema versions** — every shard at expected version.
4. **Worker health** — every worker process alive and not in restart-loop.
5. **SLA snapshot** — uptime, p50/p95/p99, cache hit rate, disk usage,
   queue depth.

Returns per-check status and a list of remediation recommendations.

## Suggested workflow

- After install: `/dt-doctor` to confirm the daemon is healthy.
- When anything feels slow: `/dt-doctor` to see which worker is degraded.
- In CI: `mneme doctor --json | jq .ok` (exit 0 only if `ok = true`).

If any check fails, the report includes a recommendation. The most common
fix is `mneme daemon restart`.

See also: `/dt-rebuild` (last resort — re-parse from scratch) and the
`mneme-doctor` sub-agent.
