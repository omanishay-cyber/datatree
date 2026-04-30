# Mneme for VS Code

Mneme is a local-first persistent memory and code graph for AI agents. It indexes
your workspace, surfaces drift via auditors, and exposes a Model Context Protocol
(MCP) server so agents like Copilot Chat or Claude Code can recall facts and
relationships across sessions instead of starting from scratch each time.

This extension is a thin shell around the `mneme` CLI. It does not bundle the
binary - you install `mneme` yourself, and the extension wires it into VS Code.

## What this extension does

- **Auto-registers the MCP server** on activation by calling
  `mneme register-mcp --platform vscode`. Idempotent. Disable with
  `mneme.autoRegisterMCP: false` if you prefer to manage it manually.
- **Status bar item** showing daemon health, polled every 30 seconds. Click it
  to run `mneme doctor`.
- **Command palette entries** for the most common operations:
  - `Mneme: Build current workspace`
  - `Mneme: Doctor`
  - `Mneme: Recall`
  - `Mneme: Open Live Graph (Vision)`
  - `Mneme: Start daemon`
  - `Mneme: Stop daemon`
- **Output channel** named `Mneme` that captures stderr from spawned commands.

## Installation

1. Install the `mneme` CLI on your machine. See the
   [main repo](https://github.com/omanishay-cyber/mneme) for instructions.
2. Verify it works: `mneme doctor` should print a green checklist.
3. Install this extension. Either:
   - Install from the VS Code Marketplace (search for "Mneme"), or
   - Install from VSIX: `Extensions` view -> `...` menu -> `Install from VSIX...`.
4. Reload VS Code. The status bar should show `mneme` once the daemon is up.

## Settings

| Setting | Default | What it does |
|---|---|---|
| `mneme.binaryPath` | `mneme` | Path to the mneme binary. Set an absolute path if VS Code can't find it on PATH. |
| `mneme.autoRegisterMCP` | `true` | Run `mneme register-mcp --platform vscode` on activation. |

## Screenshots

_(GIF placeholder)_ Recall in action.

_(GIF placeholder)_ Build with progress streaming to the output panel.

## Troubleshooting

- **Status bar shows `mneme down`**: the daemon is not running. Run
  `Mneme: Start daemon` from the palette, or check `mneme doctor`.
- **`mneme: command not found`**: VS Code's environment PATH doesn't include
  the install location. Set `mneme.binaryPath` to an absolute path.
- **MCP server doesn't show up in Copilot Chat / Claude Code**: run
  `mneme register-mcp --platform vscode` in a terminal and check the output
  for the registration target it modified.

## License

Apache-2.0. See [LICENSE](LICENSE).

This extension's source lives in the main mneme repo under `vscode/`.
