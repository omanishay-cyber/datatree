# plugin/templates — reference templates only

This directory contains a small set of **reference templates** showing
the unique config-file shapes that some platforms use. The templates
here are **not** the source of truth for what `mneme install` actually
writes.

## Source of truth

The canonical MCP server entry written into every platform's config
file is generated dynamically by Rust. See:

- `cli/src/platforms/mod.rs::mneme_mcp_entry()` — the single function
  every platform adapter calls to build the JSON / TOML / YAML block
  that gets injected into that platform's config.
- `cli/src/platforms/<platform>.rs` — one file per platform, calls
  `mneme_mcp_entry()` and shapes the result for that platform's
  schema (Codex needs TOML, Cursor wants `.cursor/mcp.json`, etc.).

When you bump the canonical MCP entry, edit `mneme_mcp_entry()`. Do
**not** edit the templates in this directory — they are not consumed
by the installer at runtime.

## Why some templates remain

A handful of platforms (Copilot's `.github/copilot-instructions.md`,
Aider's `.aider.conf.yml`, Codex's `config.toml`, etc.) have a config
shape that's wider than the MCP block alone. The templates that remain
exist so a contributor can read one canonical example without having
to hunt through the writer code.

If you're adding a new platform, follow this checklist:

1. Add a `Platform::<New>` variant in `cli/src/platforms/mod.rs`.
2. Add `cli/src/platforms/<platform>.rs` implementing the writer.
3. Have the writer call `mneme_mcp_entry()` for the MCP block.
4. (Optional) Drop a reference template here so future readers can see
   the wider config shape. **Do not** rely on this template at runtime.

## Removed sub-directories

The following platform subdirectories used to live here as templates and
have been removed because they were either redundant (the canonical
MCP entry covers them) or stale:

- antigravity, claude-code, cursor, factory-droid, hermes, kiro,
  openclaw, qoder, trae, windsurf

Their writers in `cli/src/platforms/<platform>.rs` continue to work —
they call `mneme_mcp_entry()` directly and never read this directory.
