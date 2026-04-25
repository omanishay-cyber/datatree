# Plugin templates

This folder holds **per-platform manifest stubs** that mneme writes to a
host AI tool when you run `mneme install` or `mneme register-mcp`.

## What lives here

For each of the 19 supported AI platforms, the per-platform sub-folder
mirrors the real on-disk layout the platform expects. So:

```
claude-code/CLAUDE.md.template
codex/AGENTS.md.template
codex/config.toml.template
copilot/.vscode/mcp.json.template
copilot/.github/copilot-instructions.md.template
cursor/.cursor/rules/mneme.mdc.template
cursor/.cursor/hooks.json.template
cursor/.cursorrules.template
gemini-cli/GEMINI.md.template
hermes/AGENTS.md.template
kiro/.kiro/steering/mneme.md.template
opencode/.opencode/plugins/mneme.ts.template
qoder/QODER.md.template
qwen/QWEN.md.template
windsurf/.windsurfrules.template
zed/AGENTS.md.template
... etc.
```

These are **manifest templates** — instructions, rule files, plugin
loaders. mneme renders them at install time with `{{mneme_root}}`,
`{{mneme_home}}`, `{{project_root}}` variables substituted.

## Where the MCP server entry comes from

The `mcpServers.mneme` block written into each platform's MCP config
JSON file is **not** read from a `.mcp.json.template` file. The
canonical entry is generated at runtime by:

  `cli/src/platforms/mod.rs::mneme_mcp_entry()`

This is the single source of truth. It produces a JSON object shaped
like:

```json
{
  "command": "mneme",
  "args": ["mcp", "stdio"],
  "env": {
    "MNEME_LOG": "info"
  },
  "transport": "stdio"
}
```

`merge_mcp_json_object()` and `merge_mcp_json_array()` (same file) splice
that entry into whatever existing JSON the host already has, preserving
unrelated `mcpServers` keys. There is no template-rendering pass for the
MCP entry — just one canonical JSON literal that ships in the binary.

This was historically not the case; mneme used to ship 10 byte-identical
`.mcp.json.template` files (one per platform that uses the
`mcpServers`-object schema) as documentation. They were removed in
v0.3.1 (audit-L10) because:

1. They were never read by the install code at runtime.
2. Their content drifted from `mneme_mcp_entry()` (used to say
   `command: "bun"`, the real entry says `command: "mneme"`).
3. Anyone editing a template would be surprised when their change
   had no effect.

## What's left in this folder for MCP

Just `copilot/.vscode/mcp.json.template`. VS Code Copilot uses a
**different** top-level key (`servers` instead of `mcpServers`) for
historical reasons. Until that platform's runtime path also routes
through `mneme_mcp_entry()`, the template is kept here as a reference
artifact. (TODO: fold Copilot into the unified path; the schema
difference is declarative.)

## Adding a new platform

1. Add a sub-folder named after the platform (`my-tool/`).
2. Drop in any **manifest** templates the platform expects (rules,
   agent instructions, plugin loaders). These get the standard
   `{{mneme_root}}` / `{{mneme_home}}` / `{{project_root}}` placeholders.
3. **Do not** add a `.mcp.json.template`. The MCP server entry is
   handled by `cli/src/platforms/<my_tool>.rs` returning the right
   `mcp_config_path()` and `mcp_format()`.
4. Implement the adapter in `cli/src/platforms/<my_tool>.rs` and
   register it in the `Platform` enum in `cli/src/platforms/mod.rs`.

That's it — the install code does the rest.

## Why marker injection

Manifest templates use a marker block:

```text
<!-- mneme-start v1.0 -->
…template body…
<!-- mneme-end -->
```

`MarkerInjector` (in `cli/src/markers.rs`) is **idempotent**: re-running
install replaces only the content between the markers, leaving every
unrelated user edit untouched. `mneme uninstall` strips the block
cleanly.
