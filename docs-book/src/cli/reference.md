# CLI commands

Mneme's CLI is intentionally thin — every subcommand maps 1:1 to a handler in `cli/src/commands/<name>::run`. Errors bubble as `CliError` with a stable exit code that hooks and shells can branch on. Use `mneme <subcommand> --help` for full args on any command.

## Index

| Command | Purpose |
|---|---|
| `mneme install` | Register Mneme with an AI host (Claude Code, Cursor, Codex). |
| `mneme uninstall` | Reverse `install`. |
| `mneme rollback` | Restore from an install receipt. |
| `mneme register-mcp` | Register only the MCP entry, no hooks. |
| `mneme unregister-mcp` | Reverse `register-mcp`. |
| `mneme self-update` | Replace the binary set with the latest GitHub release. |
| `mneme update` | Incremental re-index (only files changed since last build). |
| `mneme build` | Full project ingest. |
| `mneme rebuild` | Drop everything, re-parse from scratch. |
| `mneme status` | Graph stats, drift count, last build time. |
| `mneme doctor` | Full health check + bundled model inventory. |
| `mneme view` | Open the vision SPA (Tauri or browser). |
| `mneme audit` | Run all configured scanners. |
| `mneme drift` | Show current drift findings. |
| `mneme recall` | Semantic recall against history / decisions / concepts / files. |
| `mneme blast` | Blast radius for a file or function. |
| `mneme why` | Decision trace from ledger + git + concept graph. |
| `mneme history` | Search the conversation history. |
| `mneme godnodes` | Top-N most-connected concepts. |
| `mneme graphify` | Multimodal extraction pass (PDF, image, audio, video, .ipynb). |
| `mneme graph-diff` | Diff two graph snapshots: nodes added/removed/modified/renamed. |
| `mneme export` | Export graph to GraphML / Obsidian / Cypher / SVG / JSON-LD. |
| `mneme snap` | Manual snapshot of the active shard. |
| `mneme step` | Step Ledger ops (plan / status / show / verify / resume). |
| `mneme federated` | Federated similarity matching (opt-in). |
| `mneme models` | Local model management (install / status / path / install-onnx-runtime). |
| `mneme daemon` | Daemon control (start / stop / restart / status / logs / service-run). |
| `mneme cache` | Cache ops (du / clear). |
| `mneme abort` | Abort a running build. |
| `mneme mcp` | Run as MCP stdio server (used by AI hosts, not by humans). |
| `mneme inject` | Hook entry: UserPromptSubmit. |
| `mneme session-prime` | Hook entry: SessionStart. |
| `mneme pre-tool` | Hook entry: PreToolUse (legacy single-handler). |
| `mneme post-tool` | Hook entry: PostToolUse. |
| `mneme turn-end` | Hook entry: Stop (between turns). |
| `mneme session-end` | Hook entry: session end. |
| `mneme userprompt-submit` | Hook entry: Layer 1 self-ping (v0.4.0). |
| `mneme pretool-edit-write` | Hook entry: Layer 2 self-ping. |
| `mneme pretool-grep-read` | Hook entry: Layer 3 self-ping. |

## Most-used flow

```bash
# One-time install
mneme install --platform=claude-code

# Per-project
cd ~/your-project
mneme build .              # initial ingest
mneme view                 # open vision SPA

# Daily use
mneme update .             # incremental re-index
mneme recall "spawn"       # semantic search
mneme blast manager.rs     # impact analysis
mneme audit                # run scanners
mneme drift                # check drift findings

# When things break
mneme doctor               # full diagnostic
mneme daemon logs --tail   # tail daemon log

# Upgrade
mneme self-update          # binary update + rollback on failure
```

## Common flags

- `-v` / `-vv` / `-vvv` — log verbosity (info / debug / trace)
- `--log-json` — JSON-formatted logs (CI-friendly)
- `--socket <path>` — override IPC socket path (tests)
- Most build/update commands accept a positional project path (defaults to CWD)

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic error |
| 2 | Configuration error |
| 3 | Network error |
| 4 | Daemon unavailable |
| 5 | Schema mismatch |
| 6 | User input rejected |

The full set is documented in `cli/src/error.rs::CliError::exit_code`.

## See also

- [MCP tools](../mcp/tools.md) — what the AI sees vs what the CLI exposes
- [Hooks](../hooks/index.md) — the AI-host integration layer
- [Troubleshooting](../troubleshooting.md) — when commands go wrong
