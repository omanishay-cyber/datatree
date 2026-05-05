# Layer 2 — PreToolUse Edit / Write / MultiEdit

Fires before every Edit, Write, and MultiEdit. The only layer that may block.

## Logic

1. Extract `file_path` from the tool input.
2. Skip the gate if the file is small (`< SMALL_FILE_BYTE_THRESHOLD = 4096 bytes`) — for short files the gate's value < its cost.
3. Check if `mcp__mneme__blast_radius` was run for this file in the last `blast_radius_freshness_seconds` (default 600).
4. If yes: approve. The AI already has the impact context.
5. If no: BLOCK + auto-run `blast_radius` inline + inject the result as `additionalContext` (capped at 1500 chars).

The auto-run + inject pattern means the AI's NEXT turn can immediately retry the edit, this time informed. The block is short (one extra round-trip), and the AI gets the impact preview without the user having to remember to ask.

## Output shape — block path

```json
{
  "hook_specific": {
    "decision": "block",
    "reason": "blast_radius not run for supervisor/src/manager.rs in the last 600s. \
               Auto-running and injecting result; please retry the edit on your next turn.",
    "additionalContext": "blast_radius for supervisor/src/manager.rs: \
                          callers=12, dependents=5, tests=3, risk=high. \
                          Top callers: ..."
  }
}
```

## Output shape — approve path

```json
{ "hook_specific": { "decision": "approve" } }
```

## Source

[`cli/src/commands/pretool_edit_write.rs`][src] (Rust dispatcher) and [`mcp/src/hooks/pretool-edit-write.ts`][ts] (the design-of-record TS implementation). v0.4.0 ships the Rust path as a skeleton always-approve; the full IPC-driven gate is queued for v0.4.1 once the symbol resolver is wired into the extractor.

[src]: https://github.com/omanishay-cyber/mneme/blob/main/cli/src/commands/pretool_edit_write.rs
[ts]: https://github.com/omanishay-cyber/mneme/blob/main/mcp/src/hooks/pretool-edit-write.ts

## Configuration

```toml
[hooks]
enforce_blast_radius_before_edit = true     # default true
blast_radius_freshness_seconds = 600        # 10 min
```

To turn the gate off entirely:

```toml
[hooks]
enforce_blast_radius_before_edit = false
```

## Why the small-file bypass

For a 50-line file, blast_radius is at most 50 lines of "the file itself plus its 1-2 imports". The gate's cost (one round-trip + JSON parse + result injection) exceeds the value. The 4 KiB threshold is roughly "anything bigger than a one-screen file".
