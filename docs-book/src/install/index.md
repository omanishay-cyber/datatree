# Install

Mneme ships as a single archive per platform containing the daemon, CLI, MCP server, vision SPA, and (optionally) a bundled embedding model. Pick the route that matches your OS and AI host. All four routes install into `~/.mneme/` (or `%USERPROFILE%\.mneme\` on Windows) — the daemon binds to `127.0.0.1:7777` and the CLI lives at `~/.mneme/bin/mneme`.

## Pick a route

| Route | Best for | Auto-update? |
|----|----|----|
| `winget install Anish.Mneme` | Windows, regular updates | Yes (`winget upgrade`) |
| `winget install Anish.Mnemeos` | Windows, brand alias | Yes |
| `pip install mnemeos` | Any OS with Python | Yes (`pip install -U`) |
| `curl ... \| bash` | Linux, macOS, scripted CI | Yes (`mneme self-update`) |
| `iwr ... \| iex` | Windows without winget | Yes (`mneme self-update`) |

[Linux](./linux.md) · [macOS](./macos.md) · [Windows](./windows.md)

## What gets installed

Inside `~/.mneme/`:

```text
~/.mneme/
├── bin/
│   ├── mneme             # CLI
│   ├── mneme-daemon      # background indexer + HTTP server
│   ├── mneme-hook        # Windows GUI-subsystem hook dispatcher
│   ├── mneme-parsers     # Tree-sitter parser worker
│   ├── mneme-scanners    # audit scanner worker
│   ├── mneme-store       # storage worker
│   └── mneme-vision      # Tauri shell for the SPA
├── projects/
│   └── <hash>/           # one directory per indexed project
│       ├── graph.db
│       ├── semantic.db
│       └── ...
├── llm/
│   ├── bge-small-en-v1.5.onnx
│   └── tokenizer.json
├── static/
│   └── vision/           # SPA assets
├── config.toml           # user-editable hook settings
└── install-receipts/     # rollback metadata
```

Total disk: ~150 MB without LLM, ~3 GB with the optional Phi-3 GGUF.

## Verify

```bash
mneme --version           # mneme 0.4.0
mneme doctor              # full health check
```

Then index your first project:

```bash
cd ~/code/your-project
mneme build .
```

[First build →](../getting-started/first-build.md)

## Hook integration

After install, register Mneme with your AI host. For Claude Code:

```bash
mneme install --platform=claude-code
```

This writes 3 hooks to `~/.claude/settings.json` (UserPromptSubmit, PreToolUse Edit/Write, PreToolUse Grep/Read) and the MCP server entry. Restart Claude Code; the panel should show `mneme: connected` with 50 tools.

[Hook details →](../hooks/index.md) · [MCP tool inventory →](../mcp/tools.md)

## Uninstall

```bash
mneme uninstall --platform=claude-code   # remove host integration
mneme uninstall                          # nuke ~/.mneme/ entirely
```

The first form leaves your indexes intact; the second deletes everything. Both are reversible via `mneme rollback` if you have an install receipt under `install-receipts/`.

## Upgrade

```bash
mneme self-update          # download + verify + atomic swap
                           # health-checks the new binary;
                           # restores from .old backup on failure
```

The post-swap verification was added in v0.4.0 — if `mneme --version` doesn't exit 0 within 5 s on the new binary, every `.old` backup is restored over the new one. You're never left with a broken install.

[Auto-update internals →](../releases/v0.4.0.md#auto-update-apply-mode)
