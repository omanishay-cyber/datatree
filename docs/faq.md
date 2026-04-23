# FAQ

## The big-picture questions

### What is datatree trying to fix?

Every AI coding assistant has the same three flaws:

1. **Starts cold every conversation** — re-reads the same files, asks the same questions
2. **Loses its place when context compacts** — you give Claude a 100-step plan and at step 50 the conversation compresses; Claude forgets and starts over
3. **Drifts from your rules** — your `CLAUDE.md` says "no hardcoded colors", but five prompts later Claude hardcodes one

datatree fixes all three at the architecture level (not the prompt level) by externalising the memory into a local SQLite graph that gets silently fed back into Claude's context each turn.

### Is this just another RAG system?

No. RAG chunks your documents and does embedding lookup when you ask a question. datatree is doing the opposite: it tracks **structured state** (numbered steps, decisions, constraints, verbatim conversation turns, file-by-file graph edges) and **proactively injects** the correct slice into every turn — before Claude has even thought to ask.

datatree does have an embeddings store and supports semantic recall, but that's one tool among 33+. The headline feature is the Step Ledger, which is not RAG at all — it's a verified state machine.

### Do you send my code to a server?

No. datatree runs **100% locally**. No cloud, no telemetry, no API keys, no "phone home" on startup, no embedded analytics. Models are CPU-only and either bundled with the binaries or downloaded once from a path you specify. You can block datatree at your firewall and it will keep working.

### How is this different from code-review-graph or graphify?

- **code-review-graph** (CRG) is the state-of-the-art deterministic code graph. datatree's structural graph builds on the same idea (Tree-sitter AST → SQLite) but adds 26 more storage layers, compaction resilience, and the Step Ledger. Benchmarks indicate ~3× better token reduction than CRG's 8.2× average.
- **graphify** is a multimodal knowledge-graph builder that uses LLM subagents to extract concepts from PDFs/audio/video. datatree absorbs graphify's multimodal pipeline as one of its workers — they're complementary, not competing.

See the README's benchmark table for a feature-by-feature comparison.

---

## Installation & setup

### Why do I need Rust, Bun, and Python all three?

Each is used for what it's best at:

- **Rust** — supervisor, storage, parsers, scanners. Must be fast, fault-tolerant, and statically linkable.
- **Bun + TypeScript** — MCP server and vision app. Hot-reloadable tool definitions; `bun:sqlite` is the fastest SQLite binding in any runtime.
- **Python** — multimodal sidecar. PDF/OCR/Whisper ecosystems are irreplaceable here.

The v0.1.1 release will ship prebuilt binaries so you don't need the toolchains yourself — just the runtimes.

### Install failed. What do I check?

Walk down [`INSTALL.md`'s troubleshooting section](../INSTALL.md#troubleshooting). The most common causes:

1. **Rust not on PATH** — reopen your terminal after installing Rust
2. **Build tools missing on Windows** — `winget install Microsoft.VisualStudio.2022.BuildTools`
3. **Bun not found** — `winget install Oven-sh.Bun`
4. **Python too old** — need 3.10+ for the multimodal sidecar

### Where is my data stored?

Everything lives under `~/.datatree/`:

- `~/.datatree/projects/<sha>/` — per-project shards (one folder per project)
- `~/.datatree/snapshots/` — hourly rolling snapshots of each shard
- `~/.datatree/cache/` — embedding cache, docs cache, multimodal cache
- `~/.datatree/bin/` — the worker binaries
- `~/.datatree/logs/` — supervisor + worker logs

Remove the folder and datatree is gone. Nothing lives anywhere else.

### Does datatree slow down my machine?

The supervisor uses ~30–80 MB RAM idle. During active indexing it'll push one CPU core for a few seconds. Parser workers stay resident but idle between jobs (a few MB each). The daemon is designed to be invisible when nothing's happening.

---

## Claude Code integration

### How does Claude know datatree is there?

When you run `datatree install`, a block gets injected into your global `~/CLAUDE.md` and an MCP server entry gets added to `~/.claude.json`. Every future Claude Code session reads the CLAUDE.md block as context and launches `datatree mcp stdio` as its MCP server. No restart of Claude Code required.

### Can I turn it off for one conversation?

Yes. In that Claude Code project, edit `.claude/settings.local.json`:

```json
{
  "mcpServers": {
    "datatree": { "enabled": false }
  }
}
```

Or delete the `<!-- datatree-start v1.0 -->` block from your CLAUDE.md temporarily.

### Does this work with Codex / Cursor / Windsurf?

Yes. `datatree install` auto-configures 18 AI tools. See the table in [README.md](../README.md#-18-supported-platforms).

### What if I use multiple AI tools on the same project?

datatree's state is per-project, not per-tool. All 18 supported tools will see the same graph, the same decisions, the same Step Ledger. You can be in Claude Code one hour and Cursor the next and everything continues.

---

## The Step Ledger

### What does "compaction-resilient" actually mean?

Claude Code's context window has a hard limit. When you fill it, the system automatically compresses older turns into a summary to free room. This is called compaction. The problem: compression loses detail. If you were on step 50 of a 100-step plan, Claude often restarts from step 30 or rereads every doc to figure out where it was.

With datatree, the Step Ledger lives in SQLite. Each step has an explicit status, a verification command, and recorded proof artifacts. When compaction happens, Claude's next turn calls `step_resume` which emits a ~5K-token bundle with the exact state: what's done, what's next, what's blocked, what constraints are active. Claude picks up at step 51.

### How do I create a Step Ledger?

Tell Claude something like *"Create a step ledger for this work"* and then write a numbered plan. Claude's tool call `step_plan_from` ingests a markdown roadmap. Or: each TaskCreate item you make becomes a step automatically if you're using datatree's wrapper.

### Can the Step Ledger span multiple conversations?

Yes. The Step Ledger is per-project, not per-conversation. You can close Claude Code, reopen it tomorrow, and the ledger state is exactly as you left it.

---

## License & commercial use

### Can I use datatree at my job?

**Yes.** Production Use includes coding / debugging / writing / research / notes at your day job. You don't need to pay anyone to run datatree as part of commercial employment.

### Can I sell datatree?

**No.** No selling copies, no selling access, no selling installations, no charging for datatree itself.

### Can I sell a product built on top of datatree?

It depends. Building *a tool whose primary value proposition is datatree* (another persistent-memory MCP, another AI superbrain, etc.) is not allowed. Building *your own product that happens to integrate datatree internally* is fine if datatree isn't the main thing being sold. When in doubt, open a GitHub Issue to discuss.

### Can I host datatree as a paid service?

**No** — that's specifically prohibited. Commercial hosting requires a separate license.

### Can I modify datatree locally?

**Yes.** Modify for your own use, write custom MCP tools, add scanners, tweak prompts — all allowed. You can't redistribute the modified version.

### What if I find a bug and want to send a fix?

PRs welcome. See [CONTRIBUTING.md](../CONTRIBUTING.md). By submitting a PR you're agreeing your contribution is licensed under the [Apache-2.0](../LICENSE).

---

## Performance & scale

### Will it work on my 100k-file monorepo?

Yes in theory. The architecture is designed for monorepo scale (WebGL visualisation handles 100k+ nodes, WAL SQLite scales to GBs of graph data, parser workers parallelise across CPU cores). In practice v0.1.0 is tested up to ~5k files; larger-repo performance tuning is ongoing.

### How much disk does it use?

About 50 MB per 10k-file project. Snapshots are rotated; worst-case disk usage is bounded.

### How much RAM?

Idle daemon: 30–80 MB. Peak during active indexing of a 10k-file project: ~500 MB across all workers. No single worker holds more than ~200 MB under normal load.

---

## Bugs & support

### Where do I report a bug?

[GitHub Issues](https://github.com/omanishay-cyber/datatree/issues) — please include OS, Rust version, Bun version, and the output of `datatree --verbose doctor`.

### Where do I ask a question?

[GitHub Discussions](https://github.com/omanishay-cyber/datatree/discussions) — any "how would I" or architectural-design question is welcome.

### Security vulnerability?

Please **do not** file a public issue. Open an Issue with `[SECURITY]` in the title and say "please contact me privately" — a maintainer will reach out via GitHub DM to continue in confidence.

---

[← back to README](../README.md)
