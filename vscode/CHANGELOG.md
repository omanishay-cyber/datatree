# Changelog

All notable changes to the Mneme VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-24

Full IDE integration. Every premium VS Code surface mneme could occupy.

### Added

- **Activity Bar + view container** with six stacked TreeViews: God Nodes, Drift
  Findings, Step Ledger, Recent Decisions, Recent Queries, Project Shards.
- **Graph Webview** (`Mneme: Open Graph View`) with live iframe to
  `mneme-livebus` and a d3-force fallback when the server is unreachable.
  Clickable nodes jump to source; supports zoom, pan, hover, and selection
  sync with the editor.
- **Hover provider** on every language, showing blast radius, recent
  decisions, drift findings, and clickable command links.
- **CodeLens provider** showing `N callers | M tests | K edits` above every
  function and class. Debounced + cached per file version.
- **Drift Diagnostics** via a `DiagnosticCollection`. Findings appear inline
  as squiggles, in the Problems panel, and as tree nodes grouped by severity.
  Polled every 15s (configurable) and on every save.
- **Context menus** in the file explorer, editor title bar, and editor body:
  recall file, blast file, show decisions.
- **Walkthrough** (`Get started with Mneme`) with 5 steps and per-view
  welcome content.
- **Settings** for every knob a power user would want: `showStatusBar`,
  `showCodeLens`, `showHover`, `showDrift`, `driftPollInterval`,
  `godNodeCount`, `logLevel`, `graphViewPort`, `notificationLevel`.
- **Live updates** via the `mneme-livebus` SSE endpoint. Reconnect with
  exponential backoff. Handles `job.complete`, `drift.finding`,
  `step.complete`, and `graph.updated` events.
- **Notifications** respecting `notificationLevel` for daemon events,
  drift criticals, and build completion.
- **Keybindings**: `Ctrl+K M R` (recall), `Ctrl+K M B` (blast under
  cursor), `Ctrl+K M G` (graph view), `Ctrl+K M D` (doctor).
- **Tree-item icons** via `ThemeIcon` + codicons. Respects theme colors.
- **Unit tests** for every output parser (19 tests). `npm test` runs the
  compiled suite; exit 0 on success.

### Changed

- `parseRecallHits` moved from `commands.ts` to `util/parse.ts`. Re-exported
  from `commands.ts` for back-compat.
- Status bar now respects `mneme.showStatusBar` and rescans on config
  change.

### Fixed

- Extension activates silently when the mneme binary isn't installed, and
  points the user to the README instead of spamming errors.

## [0.1.0] - 2026-04-24

- Initial release.
- Auto-registers the mneme MCP server with VS Code on activation.
- Adds 6 commands: Build, Doctor, Recall, Open Vision, Start/Stop daemon.
- Status bar item shows daemon health (polled every 30s).
- Honors `mneme.binaryPath` and `mneme.autoRegisterMCP` settings.
